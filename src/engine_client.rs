use crossbeam::channel;
use iceoryx2::prelude::*;
use iceoryx2_bb_posix::shared_memory::SharedMemory;
use pivot_com_types::{MAX_INLINE_DATA, MeshPublish, OP_STOP_ENGINE};
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pivot_com_types::com_types::{EngineCommand, EngineResponse};

use crate::command_thread::{CommandWork, spawn_command_thread};
use crate::mesh_sync_thread::spawn_mesh_sync_thread;

#[derive(Debug)]
struct ActiveState {
    engine_process: Child,
    command_tx: channel::Sender<CommandWork>,
    mesh_update_rx: channel::Receiver<MeshPublish>,
    shutdown: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

#[derive(Debug)]
pub struct EngineClient {
    state: Mutex<Option<ActiveState>>,
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

    pub fn send_command(&self, cmd: EngineCommand) -> Result<EngineResponse, String> {
        let (tx, rx) = channel::bounded(1);

        let guard = self.state.lock().unwrap();
        let state = guard.as_ref().ok_or("Engine not started")?;

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

    pub fn poll_mesh_sync(&self) -> Result<Option<MeshPublish>, String> {
        let guard = self.state.lock().unwrap();

        let state = match guard.as_ref() {
            Some(state) => state,
            None => return Ok(None),
        };

        match state.mesh_update_rx.try_recv() {
            Ok(publish) => Ok(Some(publish)),
            Err(_) => Ok(None),
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

        let (command_tx, command_rx) = channel::bounded::<CommandWork>(10);
        let (mesh_update_tx, mesh_update_rx) = channel::unbounded::<MeshPublish>();
        let shutdown = Arc::new(AtomicBool::new(false));
        let command_thread = spawn_command_thread(self.node.clone(), command_rx, shutdown.clone());
        let mesh_sync_thread = spawn_mesh_sync_thread(self.node.clone(), shutdown.clone(), mesh_update_tx);

        *guard = Some(ActiveState {
            engine_process,
            command_tx,
            threads: vec![command_thread, mesh_sync_thread],
            shutdown: shutdown,
            mesh_update_rx,
        });
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        {
            let guard = self.state.lock().unwrap();
            if guard.is_none() {
                return Ok(());
            }
        }

        let command = EngineCommand {
            should_cache: 1,
            op_id: OP_STOP_ENGINE,
            num_groups: 0,
            inline_data: [0; MAX_INLINE_DATA],
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
