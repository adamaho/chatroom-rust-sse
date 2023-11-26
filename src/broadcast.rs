use axum::response::sse;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{self, Sender};
use tokio::time;

pub struct Broadcast {
    inner: Mutex<Inner>,
}

#[derive(Debug, Clone, Default)]
struct Inner {
    clients: Vec<Sender<sse::Event>>,
}

impl Broadcast {
    /// Creates a new instance of Broadcast and spawns a new thread to
    /// ping connected clients to ensure they are still connected
    pub fn new() -> Arc<Self> {
        let this = Arc::new(Broadcast {
            inner: Mutex::new(Inner::default()),
        });

        Broadcast::ping(Arc::clone(&this));

        this
    }

    /// Optionally
    fn ping(this: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                time::sleep(time::Duration::from_secs(10)).await;
                this.update_clients().await;
            }
        });
    }

    async fn update_clients(&self) {
        let mut connected_clients: Vec<Sender<sse::Event>> = Vec::new();
        let clients = self.inner.lock().unwrap().clients.clone();
        for client in clients.iter() {
            if client
                .send(sse::Event::default().event("ping"))
                .await
                .is_ok()
            {
                connected_clients.push(client.clone());
            }
        }

        self.inner.lock().unwrap().clients = connected_clients;
    }

    async fn new_client(self) {}
}
