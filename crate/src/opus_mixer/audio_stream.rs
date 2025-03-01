use ogg::reading::PacketReader;
use opus::{Channels, Decoder};
use std::convert::TryInto;
use std::fmt;
use std::io::Cursor;
use std::io::{Read, Seek, SeekFrom};
use wasm_bindgen::JsValue;
use web_sys::File;

use crate::debug;
use crate::opus_mixer::drift_stats::DriftStats;
use crate::opus_mixer::{is_opus_header, is_opus_tags, CHANNELS, FRAME_SIZE, SAMPLE_RATE};
use crate::utils::read_file_to_array_buffer;

// TODO: offload to separate web workers, ala https://github.com/rustwasm/wasm-bindgen/tree/main/examples/raytrace-parallel

/// A single audio stream from an Opus file
pub struct AudioStream {
    pub(crate) packet_reader: PacketReader<Cursor<Vec<u8>>>,
    pub(crate) decoder: Option<Decoder>,
    pub(crate) header_processed: bool,
    pub(crate) comments_processed: bool,
    decoded_buffer: Vec<f32>,
    total_samples_decoded: usize,
    pub(crate) current_granule_position: i64,
    pub(crate) drift_compensation: f32,
    pub(crate) drift_stats: DriftStats,
    pub(crate) channel_count: u16, // Input channel count from the file header
}

impl fmt::Debug for AudioStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioStream")
            .field("header_processed", &self.header_processed)
            .field("comments_processed", &self.comments_processed)
            .field("total_samples_decoded", &self.total_samples_decoded)
            .field("current_granule_position", &self.current_granule_position)
            .field("drift_compensation", &self.drift_compensation)
            .field("drift_stats", &self.drift_stats)
            .field("channel_count", &self.channel_count)
            .finish()
    }
}

impl AudioStream {
    pub async fn new(file: File) -> Result<Self, JsValue> {
        let array_buffer = read_file_to_array_buffer(file).await?;
        let file_data = js_sys::Uint8Array::new(&array_buffer).to_vec();

        Ok(Self {
            packet_reader: PacketReader::new(Cursor::new(file_data)),
            decoder: None,
            header_processed: false,
            comments_processed: false,
            decoded_buffer: vec![0f32; FRAME_SIZE * CHANNELS as usize], // Initialize with stereo buffer size
            total_samples_decoded: 0,
            current_granule_position: 0,
            drift_compensation: 1.0,
            drift_stats: DriftStats::new(),
            channel_count: 1, // Default to mono, will be updated from header
        })
    }

    pub fn current_timestamp(&self) -> f64 {
        self.total_samples_decoded as f64 / SAMPLE_RATE as f64
    }

    /// Process the next packet in the stream, returning the number of samples if audio was decoded
    pub fn process_next_packet(&mut self) -> Result<Option<usize>, JsValue> {
        match self
            .packet_reader
            .read_packet()
            .map_err(|e| JsValue::from_str(&format!("Ogg read error: {}", e)))?
        {
            Some(packet) => {
                debug!("Got packet of size: {}", packet.data.len());

                // Look for headers if we haven't found them yet
                if !self.header_processed || !self.comments_processed {
                    if !self.header_processed {
                        if is_opus_header(&packet.data) {
                            debug!("Found OpusHead packet");

                            // Parse the Opus header to get the channel count
                            // OpusHead format: "OpusHead" (8 bytes) + version (1 byte) + channel_count (1 byte) + ...
                            if packet.data.len() >= 10 {
                                self.channel_count = packet.data[9] as u16;
                                debug!("Detected {} channels in input stream", self.channel_count);

                                // Resize the decoded buffer based on the input channel count
                                self.decoded_buffer =
                                    vec![0f32; FRAME_SIZE * self.channel_count as usize];
                            } else {
                                debug!("Invalid OpusHead packet, using default channel count");
                            }

                            self.header_processed = true;
                            return Ok(None);
                        } else {
                            debug!("Skipping non-header packet while looking for OpusHead");
                            return Ok(None);
                        }
                    }

                    if !self.comments_processed {
                        if is_opus_tags(&packet.data) {
                            debug!("Found OpusTags packet");
                            self.comments_processed = true;

                            // Create decoder with the correct channel count for this input stream
                            let channels = match self.channel_count {
                                1 => Channels::Mono,
                                2 => Channels::Stereo,
                                _ => {
                                    debug!(
                                        "Unsupported channel count: {}, defaulting to stereo",
                                        self.channel_count
                                    );
                                    Channels::Stereo
                                }
                            };

                            debug!("Creating decoder with {} channels", self.channel_count);

                            self.decoder =
                                Some(Decoder::new(SAMPLE_RATE, channels).map_err(|e| {
                                    JsValue::from_str(&format!("Opus decoder error: {}", e))
                                })?);

                            return Ok(None);
                        } else {
                            debug!("Skipping non-tags packet while looking for OpusTags");
                            return Ok(None);
                        }
                    }
                }

                // At this point, we should have both headers processed
                if let Some(decoder) = &mut self.decoder {
                    match decoder.decode_float(&packet.data, &mut self.decoded_buffer, false) {
                        Ok(decoded_samples) => {
                            debug!("Decoded {} samples", decoded_samples);
                            self.total_samples_decoded += decoded_samples;
                            self.current_granule_position += decoded_samples as i64;
                            Ok(Some(decoded_samples))
                        }
                        Err(e) => {
                            eprintln!("Error decoding packet: {}", e);
                            Ok(None)
                        }
                    }
                } else {
                    debug!("No decoder available for audio packet");
                    Ok(None)
                }
            }
            None => {
                debug!("End of stream reached");
                Ok(None)
            }
        }
    }

    pub fn get_decoded_samples(&self) -> &[f32] {
        &self.decoded_buffer
    }

    /// Get the channel count of this input stream (1 for mono, 2 for stereo)
    pub fn get_channel_count(&self) -> u16 {
        self.channel_count
    }

    /// Seek to a target timestamp using bisection search as specified in RFC 7845
    pub fn seek_to_timestamp(&mut self, target_timestamp: f64) -> Result<(), JsValue> {
        let target_granule = (target_timestamp * SAMPLE_RATE as f64) as i64;
        debug!(
            "Seeking to granule position {} ({:.2}s)",
            target_granule, target_timestamp
        );

        // Get file size for bisection bounds
        let file = self.packet_reader.get_mut();
        let file_size = file
            .seek(SeekFrom::End(0))
            .map_err(|e| JsValue::from_str(&format!("Seek error: {}", e)))?;

        // Initialize bisection search bounds
        let mut left = 0;
        let mut right = file_size;
        let mut last_granule = 0;
        let mut best_position = 0;

        // Bisection search for the target granule position
        while right - left > 4096 {
            // Stop when we're within a page
            let mid = left + (right - left) / 2;
            file.seek(SeekFrom::Start(mid))
                .map_err(|e| JsValue::from_str(&format!("Seek error: {}", e)))?;

            // Sync to next page boundary
            let mut buf = [0u8; 4];
            let mut capture_pattern_found = false;
            while !capture_pattern_found
                && file
                    .stream_position()
                    .map_err(|e| JsValue::from_str(&format!("Seek error: {}", e)))?
                    < right
            {
                match file.read_exact(&mut buf[..1]) {
                    Ok(_) => {
                        if buf[0] == 'O' as u8 {
                            if let Ok(_) = file.read_exact(&mut buf[1..]) {
                                if &buf == b"OggS" {
                                    capture_pattern_found = true;
                                    file.seek(SeekFrom::Current(-4)).map_err(|e| {
                                        JsValue::from_str(&format!("Seek error: {}", e))
                                    })?; // Rewind to start of page
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }

            if !capture_pattern_found {
                // No page found after mid, search in first half
                right = mid;
                continue;
            }

            // Read page header
            let mut header = [0u8; 27];
            file.read_exact(&mut header)
                .map_err(|e| JsValue::from_str(&format!("Read error: {}", e)))?;

            // Extract granule position (bytes 6-13, little endian)
            let granule = i64::from_le_bytes(header[6..14].try_into().unwrap());

            if granule < 0 {
                // Headers or invalid granule, search in second half
                left = mid;
                continue;
            }

            debug!("Found granule {} at position {}", granule, mid);

            // Update search bounds based on granule position
            if granule < target_granule {
                left = mid;
                if granule > last_granule {
                    last_granule = granule;
                    best_position = file
                        .stream_position()
                        .map_err(|e| JsValue::from_str(&format!("Seek error: {}", e)))?
                        - header.len() as u64;
                }
            } else {
                right = mid;
                if granule < last_granule || last_granule == 0 {
                    last_granule = granule;
                    best_position = file
                        .stream_position()
                        .map_err(|e| JsValue::from_str(&format!("Seek error: {}", e)))?
                        - header.len() as u64;
                }
            }
        }

        // Seek to best position found
        debug!(
            "Seeking to best position: {} (granule: {})",
            best_position, last_granule
        );

        // Get the file handle and seek to start
        let file = self.packet_reader.get_mut();
        file.seek(SeekFrom::Start(0))
            .map_err(|e| JsValue::from_str(&format!("Seek error: {}", e)))?;

        // Reset decoder state
        self.decoder = None;
        self.header_processed = false;
        self.comments_processed = false;

        // Process until we find the OpusHead and OpusTags headers
        while !self.header_processed || !self.comments_processed {
            match self.process_next_packet()? {
                Some(_) => {}
                None => {
                    if self.header_processed && self.comments_processed {
                        break;
                    }
                }
            }
        }

        // Now seek to the target position
        let file = self.packet_reader.get_mut();
        file.seek(SeekFrom::Start(best_position))
            .map_err(|e| JsValue::from_str(&format!("Seek error: {}", e)))?;

        // Update the total samples decoded based on the granule position
        self.total_samples_decoded = (last_granule as f64 * SAMPLE_RATE as f64 / 48000.0) as usize;

        Ok(())
    }
}
