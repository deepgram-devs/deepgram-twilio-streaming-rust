#[derive(Clone)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close(Option<tungstenite::protocol::CloseFrame<'static>>),
}

impl From<axum::extract::ws::Message> for Message {
    fn from(item: axum::extract::ws::Message) -> Self {
        match item {
            axum::extract::ws::Message::Text(text) => Message::Text(text),
            axum::extract::ws::Message::Binary(binary) => Message::Binary(binary),
            axum::extract::ws::Message::Ping(ping) => Message::Ping(ping),
            axum::extract::ws::Message::Pong(pong) => Message::Pong(pong),
            // will deal with this later
            axum::extract::ws::Message::Close(_) => Message::Close(None),
        }
    }
}

impl From<tokio_tungstenite::tungstenite::Message> for Message {
    fn from(item: tokio_tungstenite::tungstenite::Message) -> Self {
        match item {
            tokio_tungstenite::tungstenite::Message::Text(text) => Message::Text(text),
            tokio_tungstenite::tungstenite::Message::Binary(binary) => Message::Binary(binary),
            tokio_tungstenite::tungstenite::Message::Ping(ping) => Message::Ping(ping),
            tokio_tungstenite::tungstenite::Message::Pong(pong) => Message::Pong(pong),
            // will deal with this later
            tokio_tungstenite::tungstenite::Message::Close(_) => Message::Close(None),
        }
    }
}

impl From<Message> for axum::extract::ws::Message {
    fn from(item: Message) -> axum::extract::ws::Message {
        match item {
            Message::Text(text) => axum::extract::ws::Message::Text(text),
            Message::Binary(binary) => axum::extract::ws::Message::Binary(binary),
            Message::Ping(ping) => axum::extract::ws::Message::Ping(ping),
            Message::Pong(pong) => axum::extract::ws::Message::Pong(pong),
            // will deal with this later
            Message::Close(_) => axum::extract::ws::Message::Close(None),
        }
    }
}

impl From<Message> for tokio_tungstenite::tungstenite::Message {
    fn from(item: Message) -> tokio_tungstenite::tungstenite::Message {
        match item {
            Message::Text(text) => tokio_tungstenite::tungstenite::Message::Text(text),
            Message::Binary(binary) => tokio_tungstenite::tungstenite::Message::Binary(binary),
            Message::Ping(ping) => tokio_tungstenite::tungstenite::Message::Ping(ping),
            Message::Pong(pong) => tokio_tungstenite::tungstenite::Message::Pong(pong),
            // will deal with this later
            Message::Close(_) => tokio_tungstenite::tungstenite::Message::Close(None),
        }
    }
}
