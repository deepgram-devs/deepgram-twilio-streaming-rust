use serde::{Deserialize, Serialize};

// Twilio responses (mostly)
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Event {
    pub event: String,
    #[serde(rename = "sequenceNumber")]
    pub sequence_number: String,
    #[serde(flatten)]
    pub event_type: EventType,
    #[serde(rename = "streamSid")]
    pub stream_sid: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum EventType {
    #[serde(rename = "start")]
    Start(EventStart),
    #[serde(rename = "media")]
    Media(EventMedia),
}

impl Default for EventType {
    fn default() -> Self {
        EventType::Media(Default::default())
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct EventStart {
    #[serde(rename = "accountSid")]
    pub account_sid: String,
    #[serde(rename = "streamSid")]
    pub stream_sid: String,
    #[serde(rename = "callSid")]
    pub call_sid: String,
    pub tracks: Vec<String>,
    #[serde(rename = "mediaFormat")]
    pub media_format: MediaFormat,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MediaFormat {
    pub encoding: String,
    #[serde(rename = "sampleRate")]
    pub sample_rate: u32,
    pub channels: u32,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct EventMedia {
    pub track: String,
    pub chunk: String,
    pub timestamp: String,
    pub payload: String,
}
