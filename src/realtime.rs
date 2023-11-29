use std::{collections::HashMap, convert::Infallible};

use anyhow::Result;
use axum::response::sse::Event;
use futures_util::future;
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;

#[derive(Debug, Clone)]
pub struct RealtimeClient {
    pub id: String,
    pub tx: Sender<Result<Event, Infallible>>,
}

impl RealtimeClient {
    pub fn with_stream(
        id: String,
        buffer: usize,
    ) -> (ReceiverStream<Result<Event, Infallible>>, Self) {
        let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(buffer);
        let stream = ReceiverStream::new(rx);
        (stream, RealtimeClient { id, tx })
    }
}

pub enum RealtimeEvent {
    Connected((String, RealtimeClient)),
    Emit((String, String)),
}

#[derive(Default)]
struct Realtime {
    clients: HashMap<String, RealtimeClient>,
}

impl Realtime {
    /// Adds a new client to the chatroom
    pub fn add_client(&mut self, client: RealtimeClient) {
        self.clients
            .insert(client.id.clone(), client);
    }

    /// Sends a message to every client in the chatroom
    pub async fn send_message(&mut self, message: String) -> Result<()> {
        let mut message_futures = Vec::new();
        let mut stale_clients = Vec::new();

        for client in self.clients.iter() {
            let (id, client) = client;
            if client.tx.is_closed() {
                let id = id;
                stale_clients.push(id.clone());
                continue;
            }

            message_futures.push(
                client
                    .tx
                    .send(Ok(Event::default().event("message").data(message.clone()))),
            )
        }

        future::join_all(message_futures).await;

        // remove stale clients
        for id in stale_clients.iter() {
            self.clients.remove(id);
        }

        Ok(())
    }
}


pub fn sender(buffer: usize) -> Result<Sender<RealtimeEvent>> {
    let (tx, mut events) = mpsc::channel(buffer);

    tokio::spawn(async move {
        let mut chatroom = Realtime::default();
        loop {
            match events.recv().await {
                Some(event) => match event {
                    RealtimeEvent::Connected((_store_key, sender)) => {
                        chatroom.add_client(sender);
                    }
                    RealtimeEvent::Emit((_store_key, message)) => {
                        chatroom.send_message(message).await.unwrap();
                    }
                },
                None => {
                    eprintln!("[ERROR]: an unknown chatroom event error occurred",);
                    break;
                }
            }
        }
    });

    Ok(tx)
}
