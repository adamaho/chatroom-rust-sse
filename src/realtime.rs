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
    stores: HashMap<String, HashMap<String, RealtimeClient>>,
}

impl Realtime {
    /// Adds a new client to the chatroom
    pub fn add_client(&mut self, store_key: String, client: RealtimeClient) {
        self.stores
            .entry(store_key)
            .and_modify(|store| {
                store.insert(client.id.clone(), client.clone());
            })
            .or_insert_with(|| {
                let mut store = HashMap::new();
                store.insert(client.id.clone(), client.clone());
                store
            });

        println!("stores: {:?}", self.stores);
    }

    /// Send message to every client in the store
    pub async fn send_message(&mut self, store_key: String, message: String) -> Result<()> {
        let mut message_futures = Vec::new();
        let mut stale_clients = Vec::new();

        if let Some(clients) = self.stores.get_mut(&store_key) {
            for client in clients.iter() {
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
                clients.remove(id);
            }
        }

        Ok(())
    }
}

pub fn sender(buffer: usize) -> Result<Sender<RealtimeEvent>> {
    let (tx, mut events) = mpsc::channel(buffer);

    tokio::spawn(async move {
        let mut realtime = Realtime::default();
        loop {
            match events.recv().await {
                Some(event) => match event {
                    RealtimeEvent::Connected((store_key, sender)) => {
                        realtime.add_client(store_key, sender);
                    }
                    RealtimeEvent::Emit((store_key, message)) => {
                        realtime.send_message(store_key, message).await.unwrap();
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
