use iceoryx2::prelude::*;
use std::process::Child;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::com_types::{EngineCommand, EngineResponse};

const COMMAND_SERVICE_NAME: &str = "PivotEngine/CommandService";
const MESH_UPDATES_SERVICE_NAME: &str = "PivotEngine/MeshUpdates";
const NOTIFICATIONS_SERVICE_NAME: &str = "PivotEngine/Notifications";

// Fixed-size strings are essential for Zero-Copy structs
struct CommandWork {
    cmd: EngineCommand,
    // A one-shot channel to send the response back to the caller
    response_tx: mpsc::Sender<Result<EngineResponse, String>>,
}

#[derive(Debug)]
struct ActiveState {
    engine_process: Child,
    // The iceoryx2 client port for sending commands
    command_tx: mpsc::SyncSender<CommandWork>,
}

#[derive(Debug)]
pub struct EngineClient {
    state: Arc<Mutex<Option<ActiveState>>>,
    // The node is the identity of this process in the iceoryx2 network
    node: Arc<Node<ipc::Service>>,
}

impl EngineClient {
    pub fn new() -> Self {
        let node = NodeBuilder::new()
            .create::<ipc::Service>()
            .expect("Failed to create iceoryx2 node");

        EngineClient {
            state: Arc::new(Mutex::new(None)),
            node: Arc::new(node),
        }
    }

    pub fn start(&self, path: String) -> Result<(), String> {
        let mut guard = self.state.lock().unwrap();

        if guard.is_some() {
            return Ok(());
        }

        let engine_process = std::process::Command::new(path)
            .spawn()
            .map_err(|e| e.to_string())?;

        let (command_tx, command_rx) = mpsc::sync_channel::<CommandWork>(10);
        let node_handle = self.node.clone();
        Self::spawn_command_service(node_handle, command_rx);

        *guard = Some(ActiveState {
            engine_process,
            command_tx,
        });
        Ok(())
    }

    pub fn send_command(&self, cmd: EngineCommand) -> Result<EngineResponse, String> {
        let guard = self.state.lock().unwrap();

        let state = guard.as_ref().ok_or("Engine not started")?;

        let (tx, rx) = mpsc::channel();
        state
            .command_tx
            .send(CommandWork {
                cmd,
                response_tx: tx,
            })
            .map_err(|e| format!("Failed to send command: {}", e))?;

        drop(guard);

        rx.recv()
            .map_err(|e| format!("Failed to receive response: {}", e))?
    }

    pub fn listen_for_updates(&self) {
        let node = self.node.clone();

        thread::spawn(move || {
            // 1. Create independent ports for this thread
            // This ensures we never compete with send_command for a Mutex.

            let (subscriber, listener) = loop {
                let sub_service = node
                    .service_builder(&MESH_UPDATES_SERVICE_NAME.try_into().unwrap())
                    .publish_subscribe::<[u8]>()
                    .open();

                let event_service = node
                    .service_builder(&NOTIFICATIONS_SERVICE_NAME.try_into().unwrap())
                    .event()
                    .open();

                match (sub_service, event_service) {
                    (Ok(sub), Ok(event)) => {
                        break (
                            sub.subscriber_builder().create().expect("Subscriber error"),
                            event.listener_builder().create().expect("Listener error"),
                        );
                    }
                    _ => {
                        // Engine isn't fully ready yet, or services aren't registered.
                        // Sleep for a bit and try again.
                        thread::sleep(Duration::from_millis(500));
                        println!("Waiting for Engine mesh services to appear...");
                    }
                }
            };

            let waitset = WaitSetBuilder::new().create::<ipc::Service>().unwrap();
            let _guard = waitset.attach_notification(&listener).unwrap();

            println!("Background mesh sync loop active.");

            loop {
                // Blocks here until the Engine signals the listener
                if waitset
                    .wait_and_process(|_| CallbackProgression::Stop)
                    .is_err()
                {
                    break;
                }

                // Drain all pending samples from the subscriber
                while let Ok(Some(sample)) = subscriber.receive() {
                    // This is zero-copy reading from the Engine's cache
                    println!("Mesh update received: {} bytes", sample.len());
                    // Logic for processing mesh (e.g. updating a local renderer) goes here
                }
            }
        });
    }

    fn spawn_command_service(
        node: Arc<Node<ipc::Service>>,
        command_rx: mpsc::Receiver<CommandWork>,
    ) {
        thread::spawn(move || {
            let service = loop {
                match node
                    .service_builder(&COMMAND_SERVICE_NAME.try_into().unwrap())
                    .request_response::<EngineCommand, EngineResponse>()
                    .open()
                {
                    Ok(s) => break s,
                    Err(e) => {
                        // This will tell you if the types don't match!
                        println!("Waiting for Engine command service: {:?}", e);
                    }
                }
                thread::sleep(Duration::from_millis(500));
            };

            println!("Command service loop active.");

            let iox_client = service.client_builder().create().unwrap();

            while let Ok(work) = command_rx.recv() {
                let result = (|| -> Result<EngineResponse, String> {
                    let request = iox_client
                        .loan_uninit()
                        .map_err(|e| format!("SHM loan failed: {}", e))?;
                    let pending = request
                        .write_payload(work.cmd)
                        .send()
                        .map_err(|e| format!("Send failed: {}", e))?;

                    loop {
                        if let Some(res) = pending.receive().map_err(|e| e.to_string())? {
                            return Ok(res.payload().clone());
                        }
                        // Tiny sleep to prevent 100% CPU during the microsecond wait
                        thread::sleep(std::time::Duration::from_micros(100));
                    }
                })();

                let _ = work.response_tx.send(result);
            }
        });
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut guard = self.state.lock().unwrap();

        if let Some(mut state) = guard.take() {
            let _ = state.engine_process.kill();
            let _ = state.engine_process.wait();
        }
        Ok(())
    }
}
