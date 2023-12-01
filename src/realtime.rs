use std::collections::HashMap;

use anyhow::Result;
use axum::response::sse::Event as SseEvent;
use futures_util::future;
use serde::Serialize;
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;

#[derive(Debug, Clone)]
pub struct Client {
    pub id: String,
    pub tx: Sender<Result<SseEvent, axum::Error>>,
}

impl Client {
    pub fn with_stream(
        id: String,
        buffer: usize,
    ) -> (ReceiverStream<Result<SseEvent, axum::Error>>, Self) {
        let (tx, rx) = mpsc::channel::<Result<SseEvent, axum::Error>>(buffer);
        let stream = ReceiverStream::new(rx);
        (stream, Client { id, tx })
    }
}

pub trait IntoEvent {
    fn into_event(&self) -> Result<SseEvent, axum::Error>;
}

#[derive(Serialize, Clone)]
pub struct AddEvent<T: Serialize + Clone> {
    pub data: T
}

impl<T> IntoEvent for AddEvent<T>
where
    T: Serialize + Clone,
{
    fn into_event(&self) -> Result<SseEvent, axum::Error> {
        SseEvent::default()
            .event("add".to_string())
            .json_data(&self)
    }
}

#[derive(Serialize, Clone)]
pub struct UpdateEvent<T: Serialize + Clone> {
    pub id: String,
    pub data: T,
}

impl<T> IntoEvent for UpdateEvent<T>
where
    T: Serialize + Clone,
{
    fn into_event(&self) -> Result<SseEvent, axum::Error> {
        SseEvent::default()
            .event("update".to_string())
            .json_data(&self)
    }
}

#[derive(Serialize, Clone)]
pub struct DeleteEvent {
    pub id: String,
}

impl IntoEvent for DeleteEvent {
    fn into_event(&self) -> Result<SseEvent, axum::Error> {
        SseEvent::default()
            .event("delete".to_string())
            .json_data(&self)
    }
}

#[derive(Serialize, Clone)]
pub struct AllEvent<T: Serialize + Clone> {
    pub data: Vec<T>,
}

impl<T> IntoEvent for AllEvent<T>
where
    T: Serialize + Clone,
{
    fn into_event(&self) -> Result<SseEvent, axum::Error> {
        SseEvent::default()
            .event("all".to_string())
            .json_data(&self)
    }
}

pub enum Message<T: Serialize + Clone> {
    Connected((String, Client)),
    AddEvent((String, AddEvent<T>)),
    AllEvent((String, AllEvent<T>)),
    UpdateEvent((String, UpdateEvent<T>)),
    DeleteEvent((String, DeleteEvent)),
}

#[derive(Default)]
struct Realtime {
    stores: HashMap<String, HashMap<String, Client>>,
}

impl Realtime {
    /// Adds a new client to the chatroom
    pub fn add_client(&mut self, store_key: String, client: Client) {
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
    }

    /// Send message to every client in the store
    pub async fn send_message<T: IntoEvent + Clone>(
        &mut self,
        store_key: String,
        event: T,
    ) -> Result<()> {
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

                message_futures.push(client.tx.send(event.clone().into_event()))
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

pub fn sender<T: Serialize + Clone + Send + Sync + 'static>(
    buffer: usize,
) -> Result<Sender<Message<T>>> {
    let (tx, mut events) = mpsc::channel(buffer);

    tokio::spawn(async move {
        let mut realtime = Realtime::default();
        loop {
            match events.recv().await {
                Some(event) => match event {
                    Message::Connected((store_key, sender)) => {
                        realtime.add_client(store_key, sender);
                    }
                    Message::AddEvent((store_key, event)) => {
                        realtime.send_message(store_key, event).await.unwrap();
                    }
                    Message::AllEvent((store_key, event)) => {
                        realtime.send_message(store_key, event).await.unwrap();
                    }
                    Message::UpdateEvent((store_key, event)) => {
                        realtime.send_message(store_key, event).await.unwrap();
                    }

                    Message::DeleteEvent((store_key, event)) => {
                        realtime.send_message(store_key, event).await.unwrap();
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
