use std::{collections::HashMap, convert::Infallible};

use anyhow::Result;
use axum::response::sse::Event;
use futures_util::future;
use tokio::{
    sync::mpsc::{self, Sender},
    time,
};

pub type Client = Result<Event, Infallible>;

pub enum ChatroomEvent {
    ClientConnected(Sender<Client>),
    SendMessage(String),
    Ping,
}

#[derive(Default)]
struct Chatroom {
    counter: usize,
    clients: HashMap<String, Sender<Client>>,
}

impl Chatroom {
    /// Adds a new client to the chatroom
    pub fn add_client(&mut self, sender: Sender<Client>) {
        self.counter += 1;
        self.clients
            .insert(format!("client_{:?}", self.counter), sender);
    }

    /// Sends a message to every client in the chatroom
    pub async fn send_message(&self, message: String) -> Result<()> {
        let mut message_futures = Vec::new();

        for client in self.clients.values() {
            if client.is_closed() {
                continue;
            }

            message_futures
                .push(client.send(Ok(Event::default().event("message").data(message.clone()))))
        }

        future::join_all(message_futures).await;
        Ok(())
    }

    /// Removes any stale clients that have dropped their connection
    /// to the server
    pub async fn remove_stale_clients(&mut self) {
        let mut active_clients = HashMap::new();

        for client in self.clients.iter() {
            let (id, client) = client;
            if client
                .send(Ok(Event::default().event("ping")))
                .await
                .is_ok()
            {
                active_clients.insert(id.clone(), client.clone());
            } else {
                println!("[INFO]: client {id} disconnected");
            }
        }

        self.clients = active_clients;
    }
}

pub fn channel(buffer: usize) -> Result<Sender<ChatroomEvent>> {
    let (tx, mut events) = mpsc::channel(buffer);

    tokio::spawn(async move {
        let mut chatroom = Chatroom::default();
        loop {
            match events.recv().await {
                Some(event) => match event {
                    ChatroomEvent::ClientConnected(sender) => {
                        chatroom.add_client(sender);
                        println!("[INFO]: client added");
                    }
                    ChatroomEvent::SendMessage(message) => {
                        chatroom.send_message(message).await.unwrap();
                        println!("[INFO]: message sent");
                    }
                    ChatroomEvent::Ping => {
                        chatroom.remove_stale_clients().await;
                    }
                },
                None => {
                    eprintln!("[ERROR]: an unknown chatroom event error occurred",);
                    break;
                }
            }
        }
    });

    let pinger = tx.clone();
    tokio::spawn(async move {
        loop {
            time::sleep(time::Duration::from_secs(10)).await;
            pinger.send(ChatroomEvent::Ping).await.unwrap();
        }
    });

    Ok(tx)
}
