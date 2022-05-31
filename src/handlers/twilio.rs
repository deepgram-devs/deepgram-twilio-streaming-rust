use crate::audio;
use crate::message::Message;
use crate::state::State;
use crate::twilio_response;
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use futures::channel::oneshot;
use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use std::{convert::From, sync::Arc};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

pub async fn twilio_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<State>) {
    let (_this_sender, this_receiver) = socket.split();

    // prepare the connection request with the api key authentication
    let builder = http::Request::builder()
        .method(http::Method::GET)
        .uri(&state.deepgram_url);
    let builder = builder.header("Authorization", format!("Token {}", state.api_key));
    let request = builder
        .body(())
        .expect("Failed to build a connection request to Deepgram.");

    // connect to deepgram
    let (deepgram_socket, _) = connect_async(request)
        .await
        .expect("Failed to connect to Deepgram.");
    let (deepgram_sender, deepgram_reader) = deepgram_socket.split();

    let (callsid_tx, callsid_rx) = oneshot::channel::<String>();

    tokio::spawn(handle_to_subscribers(
        Arc::clone(&state),
        callsid_rx,
        deepgram_reader,
    ));
    tokio::spawn(handle_from_twilio(
        Arc::clone(&state),
        callsid_tx,
        this_receiver,
        deepgram_sender,
    ));
}

async fn handle_to_subscribers(
    state: Arc<State>,
    callsid_rx: oneshot::Receiver<String>,
    mut deepgram_receiver: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
) {
    let callsid = callsid_rx
        .await
        .expect("Failed to receive callsid from handle_from_twilio.");

    while let Some(Ok(msg)) = deepgram_receiver.next().await {
        let mut subscribers = state.subscribers.lock().await;
        if let Some(subscribers) = subscribers.get_mut(&callsid) {
            // send the message to all subscribers concurrently
            let futs = subscribers
                .iter_mut()
                .map(|subscriber| subscriber.send(Message::from(msg.clone()).into()));
            let results = futures::future::join_all(futs).await;

            // if we successfully sent a message then the subscriber is still connected
            // other subscribers should be removed
            *subscribers = subscribers
                .drain(..)
                .zip(results)
                .filter_map(|(subscriber, result)| result.is_ok().then(|| subscriber))
                .collect();
        }
    }
}

async fn handle_from_twilio(
    state: Arc<State>,
    callsid_tx: oneshot::Sender<String>,
    mut this_receiver: SplitStream<WebSocket>,
    mut deepgram_sender: SplitSink<
        WebSocketStream<MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::Message,
    >,
) {
    let mut buffer_data = audio::BufferData {
        inbound_buffer: Vec::new(),
        outbound_buffer: Vec::new(),
        inbound_last_timestamp: 0,
        outbound_last_timestamp: 0,
    };

    // wrap our oneshot in an Option because we will need it in a loop
    let mut callsid_tx = Some(callsid_tx);
    let mut callsid: Option<String> = None;

    while let Some(Ok(msg)) = this_receiver.next().await {
        let msg = Message::from(msg);
        if let Message::Text(msg) = msg {
            let event: Result<twilio_response::Event, _> = serde_json::from_str(&msg);
            if let Ok(event) = event {
                match event.event_type {
                    twilio_response::EventType::Start(start) => {
                        // the "start" event only happens once, so having our oneshot in here is kosher
                        callsid = Some(start.call_sid.clone());

                        // sending this callsid on our oneshot will let `handle_to_subscribers` know the callsid
                        if let Some(callsid_tx) = callsid_tx.take() {
                            callsid_tx
                                .send(start.call_sid.clone())
                                .expect("Failed to send callsid to handle_to_subscribers.");
                        }

                        // make a new set of subscribers for this call, using the callsid as the key
                        state
                            .subscribers
                            .lock()
                            .await
                            .entry(start.call_sid)
                            .or_default();
                    }
                    twilio_response::EventType::Media(media) => {
                        if let Some(mixed) = audio::process_twilio_media(media, &mut buffer_data) {
                            // send the audio on to deepgram
                            if deepgram_sender
                                .send(Message::Binary(mixed).into())
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // close and remove the subscribers, if we have a callsid
    if let Some(callsid) = callsid {
        let mut subscribers = state.subscribers.lock().await;
        if let Some(subscribers) = subscribers.remove(&callsid) {
            for mut subscriber in subscribers {
                // we don't really care if this succeeds or fails as we are closing/dropping these
                let _ = subscriber.send(Message::Close(None).into()).await;
            }
        }
    }
}
