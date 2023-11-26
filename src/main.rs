use anyhow::Result;
use std::collections::HashMap;
use std::convert::Infallible;

use axum::{
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
    Router,
};
use futures_util::future;
use futures_util::stream::Stream;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_stream::wrappers::ReceiverStream;

type Client = Result<Event, Infallible>;

enum ChatroomEvent {
    ClientConnected(Sender<Client>),
    SendMessage(String),
}

#[derive(Default)]
struct Chatroom {
    counter: usize,
    clients: HashMap<String, Sender<Client>>,
}

impl Chatroom {
    pub fn add_client(&mut self, sender: Sender<Client>) {
        self.counter += 1;
        self.clients
            .insert(format!("client_{:?}", self.counter), sender);
    }

    pub async fn send_message(&mut self, message: String) -> Result<()> {
        let mut message_futures = Vec::new();

        for client in self.clients.values() {
            if client.is_closed() {
                continue;
            }

            message_futures.push(client.send(Ok(
                Event::default().event("USER_MESSAGE").data(message.clone()),
            )))
        }

        future::join_all(message_futures).await;
        self.remove_stale_clients();
        Ok(())
    }

    pub fn remove_stale_clients(&mut self) {
        let clients = self.clients.clone();
        let stale_clients = clients
            .iter()
            .filter(|(_, client)| client.is_closed())
            .map(|(id, _)| id)
            .collect::<Vec<&String>>();

        for id in stale_clients {
            self.clients.remove(id);
        }
    }
}

async fn chatroom(mut events: Receiver<ChatroomEvent>) -> Result<()> {
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
            },
            None => {
                eprintln!("[ERROR]: an unknown chatroom event error occurred",);
                break;
            }
        }
    }
    return Ok(());
}

#[tokio::main]
async fn main() {
    let (chatroom_sender, chatroom_receiver) = mpsc::channel(10);

    tokio::spawn(async {
        chatroom(chatroom_receiver).await.unwrap();
    });

    let app = Router::new()
        .route(
            "/stream",
            get({
                let sender = chatroom_sender.clone();
                move || stream(sender)
            }),
        )
        .route(
            "/send",
            get({
                let sender = chatroom_sender.clone();
                move || send(sender)
            }),
        );

    println!("Listening at http://localhost:3000");
    axum::Server::bind(&"127.0.0.1:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn stream(chatroom_sender: Sender<ChatroomEvent>) -> Sse<impl Stream<Item = Client>> {
    let (sender, receiver) = mpsc::channel::<Client>(10);

    chatroom_sender
        .send(ChatroomEvent::ClientConnected(sender))
        .await
        .unwrap();

    let rs_stream = ReceiverStream::new(receiver);

    return Sse::new(rs_stream);
}

async fn send(chatroom_sender: Sender<ChatroomEvent>) -> String {
    chatroom_sender
        .send(ChatroomEvent::SendMessage("Hello world".to_string()))
        .await.unwrap();
    return String::from("message sent.");
}
