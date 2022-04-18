use axum::{routing::get, Extension, Router};
use futures::lock::Mutex;
use std::{collections::HashMap, sync::Arc};

mod handlers;
mod message;
mod state;
mod twilio_response;

#[tokio::main]
async fn main() {
    let proxy_url = std::env::var("PROXY_URL").unwrap_or("0.0.0.0:5000".to_string());

    let deepgram_url = std::env::var("DEEPGRAM_URL")
        .unwrap_or("wss://api.deepgram.com/v1/listen?encoding=mulaw&sample_rate=8000&channels=2&multichannel=true".to_string());

    let api_key =
        std::env::var("DEEPGRAM_API_KEY").expect("Using this server requires a Deepgram API Key.");

    let state = Arc::new(state::State {
        deepgram_url,
        api_key,
        subscribers: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/twilio", get(handlers::twilio::twilio_handler))
        .route("/client", get(handlers::subscriber::subscriber_handler))
        .layer(Extension(state));

    axum::Server::bind(&proxy_url.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
