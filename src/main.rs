pub mod realtime;

use axum::{response::sse::Sse, routing::get, Router};
use futures_util::stream::Stream;
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;

#[tokio::main]
async fn main() {
    let broadcast = realtime::channel(10).unwrap();

    let app = Router::new()
        .route(
            "/stream",
            get({
                let sender = broadcast.clone();
                move || stream(sender)
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
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

async fn stream(chatroom_sender: Sender<realtime::ChatroomEvent>) -> Sse<impl Stream<Item = realtime::Client>> {
    let (sender, receiver) = mpsc::channel::<realtime::Client>(10);
    chatroom_sender
        .send(realtime::ChatroomEvent::ClientConnected(sender))
        .await
        .unwrap();
    let stream = ReceiverStream::new(receiver);
    return Sse::new(stream);
}

async fn send(chatroom_sender: Sender<realtime::ChatroomEvent>) -> String {
    chatroom_sender
        .send(realtime::ChatroomEvent::SendMessage("Hello world".to_string()))
        .await
        .unwrap();
    return String::from("message sent.");
}
