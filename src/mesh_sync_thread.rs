use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
const MESH_UPDATES_SERVICE_NAME: &str = "PivotEngine/MeshUpdates";
const NOTIFICATIONS_SERVICE_NAME: &str = "PivotEngine/Notifications";

use crossbeam::channel;
use iceoryx2::prelude::*;
use pivot_com_types::MeshPublish;

pub fn spawn_mesh_sync_thread(
    node: Arc<Node<ipc::Service>>,
    shutdown: Arc<AtomicBool>,
    mesh_update_tx: channel::Sender<MeshPublish>,
) -> std::thread::JoinHandle<()> {
    thread::spawn(move || {
        // 1. Create independent ports for this thread
        // This ensures we never compete with send_command for a Mutex.

        let (subscriber, listener) = loop {
            let sub_service = node
                .service_builder(&MESH_UPDATES_SERVICE_NAME.try_into().unwrap())
                .publish_subscribe::<MeshPublish>()
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
            let r = listener.timed_wait_all(|_| {}, Duration::from_millis(200));

            if r.is_err() {
                eprintln!("Error while waiting for mesh updates: {:?}", r.err());
            }

            // Drain all pending samples from the subscriber
            while let Ok(Some(sample)) = subscriber.receive() {
                // Send the mesh update to the main thread or whoever is interested
                let r = mesh_update_tx.send(*sample.payload());
                if r.is_err() {
                    eprintln!("Failed to send mesh update to main thread: {:?}", r.err());
                }
            }
        }
        println!("Background mesh sync loop exiting.");
    })
}
