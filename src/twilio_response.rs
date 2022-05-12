//! Definitions for the Twilio messages we need to parse

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub event: String,
    pub sequence_number: String,
    #[serde(flatten)]
    pub event_type: EventType,
    pub stream_sid: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum EventType {
    Start(EventStart),
    Media(EventMedia),
}

impl Default for EventType {
    fn default() -> Self {
        EventType::Media(Default::default())
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EventStart {
    pub account_sid: String,
    pub stream_sid: String,
    pub call_sid: String,
    pub tracks: Vec<String>,
    pub media_format: MediaFormat,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MediaFormat {
    pub encoding: String,
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
