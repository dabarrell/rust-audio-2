pub mod audio_mixer;
pub mod audio_stream;
mod drift_stats;

// Constants
pub const SAMPLE_RATE: u32 = 48000; // Opus default sample rate
pub const CHANNELS: u16 = 2; // STEREO (always use stereo output)
pub const FRAME_SIZE: usize = 960; // 20ms at 48kHz
pub const RING_BUFFER_SIZE: usize = FRAME_SIZE * 8; // Store 8 frames worth of samples

// Opus header magic signatures
const OPUS_HEAD_MAGIC: &[u8] = b"OpusHead";
const OPUS_TAGS_MAGIC: &[u8] = b"OpusTags";

pub fn is_opus_header(packet: &[u8]) -> bool {
    packet.len() >= OPUS_HEAD_MAGIC.len() && &packet[..OPUS_HEAD_MAGIC.len()] == OPUS_HEAD_MAGIC
}

pub fn is_opus_tags(packet: &[u8]) -> bool {
    packet.len() >= OPUS_TAGS_MAGIC.len() && &packet[..OPUS_TAGS_MAGIC.len()] == OPUS_TAGS_MAGIC
}
