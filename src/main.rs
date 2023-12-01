mod realtime;

use std::net::SocketAddr;

use axum::{
    extract::connect_info::ConnectInfo,
    response::sse::{self, Sse},
    routing::get,
    Router,
};
use futures_util::stream::Stream;
use serde::Serialize;
use tokio::sync::mpsc::Sender;

use crate::realtime::{Client, Message};

#[tokio::main]
async fn main() {
    let broadcast = realtime::sender(100).unwrap();

    let app = Router::new().route(
        "/stream",
        get({
            let sender = broadcast.clone();
            move |req| stream(req, sender)
        }),
    )
    .route(
        "/send",
        get({
            let sender = broadcast.clone();
            move || send(sender)
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("Listening at http://localhost:3000");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

#[derive(Serialize, Clone)]
struct Foo {
    pub bar: String,
}

async fn stream(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    sender: Sender<Message<Foo>>,
) -> Sse<impl Stream<Item = Result<sse::Event, axum::Error>>> {
    let id = addr.to_string();
    let (stream, client) = Client::with_stream(id, 10);
    sender
        .send(realtime::Message::Connected(("simple".to_string(), client)))
        .await
        .unwrap();

    return Sse::new(stream);
}

async fn send(sender: Sender<Message<Foo>>) -> String {
    let mut bar = Vec::new();

    for i in 0..10 {
        bar.push(Foo {
        bar: "baz".to_string(),
    })
    }
    let all_event = realtime::AllEvent { data: bar };
    sender
        .send(Message::AllEvent(("simple".to_string(), all_event)))
        .await
        .unwrap();
    return String::from("message sent.");
}
