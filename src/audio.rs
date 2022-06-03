use crate::twilio_response;

const MULAW_SILENCE: u8 = 0xff;
const MULAW_BYTES_PER_MS: usize = 8;
const TWILIO_MS_PER_CHUNK: usize = 20;
const MIN_TWILIO_CHUNKS_TO_MIX: usize = 20;

pub struct BufferData {
    pub inbound_buffer: Vec<u8>,
    pub outbound_buffer: Vec<u8>,
    pub inbound_last_timestamp: u32,
    pub outbound_last_timestamp: u32,
}

fn pad_with_silence(buffer: &mut Vec<u8>, current_timestamp: u32, previous_timestamp: u32) {
    let time_lost = if current_timestamp < previous_timestamp + TWILIO_MS_PER_CHUNK as u32 {
        // here we have received a timestamp that is less than TWILIO_MS_PER_CHUNK = 20 ms ahead of the previous timestamp
        // this occasionally occurs and is unexpected behavior from Twilio
        0
    } else {
        current_timestamp - (previous_timestamp + TWILIO_MS_PER_CHUNK as u32)
    };
    let silence = std::iter::repeat(MULAW_SILENCE).take(MULAW_BYTES_PER_MS * time_lost as usize);
    buffer.extend(silence);
}

/// (1) decodes twilio media events
/// (2) pads inbound and outbound buffers with silence if needed
/// (3) if there is more than MIN_TWILIO_CHUNKS_TO_MIX * TWILIO_MS_PER_CHUNK = 400 ms
///     of audio in both inbound and outbound audio buffers, drains as much audio from
///     both buffers as can be mixed together, mixes and returns this audio
pub fn process_twilio_media(
    media: twilio_response::EventMedia,
    mut buffer_data: &mut BufferData,
) -> Option<Vec<u8>> {
    // NOTE: when Twilio sends media data, it should send TWILIO_MS_PER_CHUNK = 20 ms audio chunks
    // at a time, where each ms of audio is MULAW_BYTES_PER_MS = 8 bytes
    let media_chunk = base64::decode(media.payload).unwrap();
    let media_chunk_size = media_chunk.len();
    if media_chunk_size != TWILIO_MS_PER_CHUNK * MULAW_BYTES_PER_MS {
        // here, the Twilio media chunk size is not the expected size of TWILIO_MS_PER_CHUNK * MULAW_BYTES_PER_MS bytes
        // this occasionally occurs and is unexpected behavior from Twilio
    }
    // NOTE: There are rare cases where the timestamp is less than TWILIO_MS_PER_CHUNK = 20 ms ahead of the previous chunk
    let timestamp = media.timestamp.parse::<u32>().unwrap();

    // pad the inbound or outbound buffer with silence if needed depending on timestamp info
    // and then add the audio data from the twilio media message to the buffer
    if media.track == "inbound" {
        pad_with_silence(
            &mut buffer_data.inbound_buffer,
            timestamp,
            buffer_data.inbound_last_timestamp,
        );
        buffer_data.inbound_buffer.extend(media_chunk);
        buffer_data.inbound_last_timestamp = timestamp;
    } else if media.track == "outbound" {
        pad_with_silence(
            &mut buffer_data.outbound_buffer,
            timestamp,
            buffer_data.outbound_last_timestamp,
        );
        buffer_data.outbound_buffer.extend(media_chunk);
        buffer_data.outbound_last_timestamp = timestamp;
    }

    // we will return mixed audio of MIN_TWILIO_CHUNKS_TO_MIX * TWILIO_MS_PER_CHUNK = 400 ms (or more)
    // corresponding to MIN_TWILIO_CHUNKS_TO_MIX = 20 twilio media messages (or more)
    let minimum_chunk_size = MIN_TWILIO_CHUNKS_TO_MIX * TWILIO_MS_PER_CHUNK * MULAW_BYTES_PER_MS;
    let mixable_data_size = std::cmp::min(
        buffer_data.inbound_buffer.len(),
        buffer_data.outbound_buffer.len(),
    );
    if mixable_data_size >= minimum_chunk_size {
        let mut mixed = Vec::with_capacity(mixable_data_size * 2);
        let inbound_buffer_segment = buffer_data.inbound_buffer.drain(0..mixable_data_size);
        let outbound_buffer_segment = buffer_data.outbound_buffer.drain(0..mixable_data_size);

        for (inbound, outbound) in inbound_buffer_segment.zip(outbound_buffer_segment) {
            mixed.push(inbound);
            mixed.push(outbound);
        }
        Some(mixed)
    } else {
        None
    }
}
