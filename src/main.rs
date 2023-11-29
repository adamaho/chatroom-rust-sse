pub mod realtime;

use std::{convert::Infallible, net::SocketAddr};

use axum::{
    extract::connect_info::ConnectInfo,
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use futures_util::stream::Stream;
use tokio::sync::mpsc::Sender;

#[tokio::main]
async fn main() {
    let broadcast = realtime::sender(10).unwrap();

    let app = Router::new()
        .route(
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

async fn stream(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    sender: Sender<realtime::RealtimeEvent>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let id = addr.ip().to_string();
    println!("{id}: address");

    let (stream, client) = realtime::RealtimeClient::with_stream(id, 10);

    sender
        .send(realtime::RealtimeEvent::Connected((
            "simple".to_string(),
            client,
        )))
        .await
        .unwrap();

    return Sse::new(stream);
}

async fn send(sender: Sender<realtime::RealtimeEvent>) -> String {
    sender
        .send(realtime::RealtimeEvent::Emit((
            "simple".to_string(),
            "Hello world".to_string(),
        )))
        .await
        .unwrap();
    return String::from("message sent.");
}
