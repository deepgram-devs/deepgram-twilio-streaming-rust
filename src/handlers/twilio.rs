use crate::message::Message;
use crate::state::State;
use crate::twilio_response;
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use base64::decode;
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
    let (deepgram_socket, _) = connect_async(request).await.expect("Failed.");
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

async fn handle_from_twilio(
    state: Arc<State>,
    callsid_tx: oneshot::Sender<String>,
    mut this_receiver: SplitStream<WebSocket>,
    mut deepgram_sender: SplitSink<
        WebSocketStream<MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::Message,
    >,
) {
    pub struct BufferData {
        inbound_buffer: Vec<u8>,
        outbound_buffer: Vec<u8>,
        inbound_last_timestamp: u32,
        outbound_last_timestamp: u32,
    }

    let mut buffer_data = BufferData {
        inbound_buffer: Vec::new(),
        outbound_buffer: Vec::new(),
        inbound_last_timestamp: 0,
        outbound_last_timestamp: 0,
    };

    // wrap our oneshot in an Option because we will need it in a loop
    let mut callsid_tx = Some(callsid_tx);
    let mut callsid: Option<String> = None;

    while let Some(msg) = this_receiver.next().await {
        let msg = if let Ok(msg) = msg {
            msg
        } else {
            break;
        };

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
                            // consider this unwrap
                            callsid_tx.send(start.call_sid.clone()).unwrap();
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
                        // NOTE: when Twilio sends media data, it should send 20 ms audio chunks at a time, where each ms of audio is 8 bytes
                        let media_chunk = decode(media.payload).unwrap();
                        let media_chunk_size = media_chunk.len();
                        if media_chunk_size != 20 * 8 {
                            // WARN: Twilio media chunk size is not the expected size of 20 * 8 bytes.
                        }
                        // NOTE: I've seen cases where the timestamp is less than 20 ms ahead of the previous chunk
                        let timestamp = media.timestamp.parse::<u32>().unwrap();

                        if media.track == "inbound" {
                            let time_lost = if timestamp < buffer_data.inbound_last_timestamp + 20 {
                                // WARN: Received inbound timestamp is less than 20 ms ahead of previous timestamp.
                                0
                            } else {
                                (timestamp - (buffer_data.inbound_last_timestamp + 20))
                                    .try_into()
                                    .unwrap()
                            };
                            if time_lost > 0 {
                                // silence for mulaw data is 0xff, and there are 8 bytes of mulaw data per ms
                                let silence = &vec![0xff; 8 * time_lost];
                                buffer_data.inbound_buffer.extend_from_slice(&silence[..]);
                            }
                            buffer_data
                                .inbound_buffer
                                .extend_from_slice(&media_chunk[..]);
                            buffer_data.inbound_last_timestamp = timestamp;
                        } else if media.track == "outbound" {
                            let time_lost = if timestamp < buffer_data.outbound_last_timestamp + 20
                            {
                                // WARN: Received outbound timestamp is less than 20 ms ahead of previous timestamp.
                                0
                            } else {
                                (timestamp - (buffer_data.outbound_last_timestamp + 20))
                                    .try_into()
                                    .unwrap()
                            };
                            if time_lost > 0 {
                                // silence for mulaw data is 0xff, and there are 8 bytes of mulaw data per ms
                                let silence = &vec![0xff; 8 * time_lost];
                                buffer_data.outbound_buffer.extend_from_slice(&silence[..]);
                            }
                            buffer_data
                                .outbound_buffer
                                .extend_from_slice(&media_chunk[..]);
                            buffer_data.outbound_last_timestamp = timestamp;
                        }

                        // we will send audio to deepgram once we have 400 ms (20 * 20 * 8 bytes) of audio
                        while buffer_data.inbound_buffer.len() >= 20 * 20 * 8
                            && buffer_data.outbound_buffer.len() >= 20 * 20 * 8
                        {
                            let inbound_buffer_segment: Vec<u8> =
                                buffer_data.inbound_buffer.drain(0..20 * 20 * 8).collect();
                            let outbound_buffer_segment: Vec<u8> =
                                buffer_data.outbound_buffer.drain(0..20 * 20 * 8).collect();

                            let mut mixed = Vec::new();
                            for sample in 0..20 * 20 * 8 {
                                mixed.push(inbound_buffer_segment[sample]);
                                mixed.push(outbound_buffer_segment[sample]);
                            }

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

async fn handle_to_subscribers(
    state: Arc<State>,
    callsid_rx: oneshot::Receiver<String>,
    mut deepgram_receiver: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
) {
    // consider this unwrap - what does it mean if we don't receive the callsid here?
    let callsid = callsid_rx.await.unwrap();

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
