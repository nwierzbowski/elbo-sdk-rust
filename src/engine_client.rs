use iceoryx2::prelude::*;
use pivot_com_types::{MAX_HANDLE_LEN, MAX_INLINE_DATA, OP_STOP_ENGINE};
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use pivot_com_types::com_types::{EngineCommand, EngineResponse};

const COMMAND_SERVICE_NAME: &str = "PivotEngine/CommandService";
const COMMAND_EVENT_SERVICE_NAME: &str = "PivotEngine/CommandEvents";
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
    shutdown: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
    mesh_sync_running: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct EngineClient {
    state: Mutex<Option<ActiveState>>,
    // The node is the identity of this process in the iceoryx2 network
    node: Arc<Node<ipc::Service>>,
}

impl EngineClient {
    pub fn new() -> Self {
        let node = NodeBuilder::new()
            .create::<ipc::Service>()
            .expect("Failed to create iceoryx2 node");

        EngineClient {
            state: Mutex::new(None),
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
        let shutdown = Arc::new(AtomicBool::new(false));
        let command_thread = Self::spawn_command_service(node_handle, command_rx, shutdown.clone());
        let mesh_sync_running = Arc::new(AtomicBool::new(false));

        *guard = Some(ActiveState {
            engine_process,
            command_tx,
            threads: vec![command_thread],
            shutdown: shutdown,
            mesh_sync_running,
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
        let mut guard = self.state.lock().unwrap();
        let state = match guard.as_mut() {
            Some(state) => state,
            None => panic!("Engine not started"),
        };

        let shutdown = state.shutdown.clone();
        let mesh_sync_running = state.mesh_sync_running.clone();
        let node = self.node.clone();

        if mesh_sync_running.load(Ordering::Relaxed) {
            return;
        }
        mesh_sync_running.store(true, Ordering::Relaxed);

        println!("Spawning mesh update listener thread.");
        let handle = thread::spawn(move || {
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

            println!("Background mesh sync loop active.");

            while !shutdown.load(Ordering::Relaxed) {
                // Blocks here until the Engine signals the listener
                let _ = listener.timed_wait_all(|_| {}, Duration::from_millis(200));

                // Drain all pending samples from the subscriber
                while let Ok(Some(sample)) = subscriber.receive() {
                    // This is zero-copy reading from the Engine's cache
                    println!("Mesh update received: {} bytes", sample.len());
                    // Logic for processing mesh (e.g. updating a local renderer) goes here
                }
            }
            println!("Background mesh sync loop exiting.");
        });
        state.threads.push(handle);
    }

    fn spawn_command_service(
        node: Arc<Node<ipc::Service>>,
        command_rx: mpsc::Receiver<CommandWork>,
        shutdown: Arc<AtomicBool>,
    ) -> std::thread::JoinHandle<()> {
        thread::spawn(move || {
            let (service, notifier) = loop {
                let cmd_service = node
                    .service_builder(&COMMAND_SERVICE_NAME.try_into().unwrap())
                    .request_response::<EngineCommand, EngineResponse>()
                    .open();

                let cmd_event_service = node
                    .service_builder(&COMMAND_EVENT_SERVICE_NAME.try_into().unwrap())
                    .event()
                    .open();

                match (cmd_service, cmd_event_service) {
                    (Ok(s), Ok(n)) => break (s, n),
                    _ => {
                        // Engine isn't fully ready yet, or services aren't registered.
                        // Sleep for a bit and try again.
                        thread::sleep(Duration::from_millis(500));
                        println!("Waiting for Engine command services to appear...");
                    }
                }
            };

            println!("Command service loop active.");

            let iox_client = service.client_builder().create().unwrap();
            let cmd_notifier = notifier
                .notifier_builder()
                .create()
                .expect("Failed to create Notifier");

            while !shutdown.load(Ordering::Relaxed) {
                while let Ok(work) = command_rx.recv_timeout(Duration::from_millis(200)) {
                    let result = (|| -> Result<EngineResponse, String> {
                        let request = iox_client
                            .loan_uninit()
                            .map_err(|e| format!("SHM loan failed: {}", e))?;
                        let pending = request
                            .write_payload(work.cmd)
                            .send()
                            .map_err(|e| format!("Send failed: {}", e))?;
                        // Notify the engine that a new command is available
                        cmd_notifier
                            .notify()
                            .map_err(|e| format!("Notifier failed: {}", e))?;
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
            }
            println!("Command service loop exiting.");
        })
    }

    pub fn stop(&self) -> Result<(), String> {
        {
            let guard = self.state.lock().unwrap();
            if guard.is_none() {
                return Ok(());
            }
        }

        let command = EngineCommand {
            payload_mode: 0,
            should_cache: 1,
            op_id: OP_STOP_ENGINE,
            num_groups: 0,
            inline_data: [0; MAX_INLINE_DATA],
            shm_fallback_handle: [0; MAX_HANDLE_LEN],
        };
        let res = self.send_command(command);

        let mut guard = self.state.lock().unwrap();
        if let Some(mut state) = guard.take() {
            if let Err(e) = res {
                eprintln!(
                    "Failed to send stop command to engine, killing process: {}",
                    e
                );
                let _ = state.engine_process.kill();
            }

            state.shutdown.store(true, Ordering::SeqCst);

            for handle in state.threads {
                let _ = handle.join();
            }

            let _ = state.engine_process.wait();

            println!("All threads joined. SDK is clean.");
        }

        Ok(())
    }
}
