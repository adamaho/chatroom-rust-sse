use anyhow::Result;
use std::collections::HashMap;
use std::convert::Infallible;

use axum::{
    response::sse::{Event, KeepAlive, Sse},
    response::Html,
    routing::get,
    Router,
};
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
    pub fn add_client(&mut self, sender: Sender<Client>) -> Result<()> {
        self.counter += 1;
        self.clients
            .insert(format!("client_{:?}", self.counter), sender);
        return Ok(());
    }

    pub async fn send_message(&mut self, message: String) -> Result<()> {
        let mut clients_to_remove = Vec::new();

        for (id, client) in self.clients.iter() {
            if client.is_closed() {
                clients_to_remove.push(id.clone());
            } else {
                let _ = client
                    .send(Ok(Event::default()
                        .event("USER_MESSAGE")
                        .data(message.clone())))
                    .await;
            }
        }

        if clients_to_remove.len() > 0 {
            for id in clients_to_remove.iter() {
                let _ = self.remove_client(id);
                println!("[INFO]: Client {id} disconnected");
            }
        }

        return Ok(());
    }

    pub fn remove_client(&mut self, id: &String) -> Result<()> {
        self.clients.remove(id);
        Ok(())
    }
}

async fn chatroom(mut events: Receiver<ChatroomEvent>) -> Result<()> {
    let mut chatroom = Chatroom::default();

    loop {
        match events.recv().await {
            Some(event) => match event {
                ChatroomEvent::ClientConnected(sender) => {
                    let _ = chatroom.add_client(sender);
                }
                ChatroomEvent::SendMessage(message) => {
                    let _ = chatroom.send_message(message).await;
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
        let _ = chatroom(chatroom_receiver).await;
    });

    let app = Router::new()
        .route("/", get(handler))
        .route(
            "/stream",
            get({
                let sender = chatroom_sender.clone();
                move || stream(sender)
            }),
        )
        .route("/send", get(move || send(chatroom_sender.clone())));

    println!("Listening at http://localhost:3000");
    axum::Server::bind(&"127.0.0.1:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

async fn stream(chatroom_sender: Sender<ChatroomEvent>) -> Sse<impl Stream<Item = Client>> {
    let (sender, receiver) = mpsc::channel::<Client>(10);

    match chatroom_sender
        .send(ChatroomEvent::ClientConnected(sender))
        .await
    {
        Ok(_) => {
            println!("[INFO]: client connected");
        }
        Err(err) => {
            eprintln!("[ERROR]: failed to connect client to chatroom, {:?}", err);
        }
    };

    let rs_stream = ReceiverStream::new(receiver);

    return Sse::new(rs_stream).keep_alive(KeepAlive::default());
}

async fn send(chatroom_sender: Sender<ChatroomEvent>) -> String {
    let _ = chatroom_sender
        .send(ChatroomEvent::SendMessage("Hello world".to_string()))
        .await;
    return String::from("message sent.");
}
