// use anyhow::Result;
use wasm_bindgen::JsValue;
use web_sys::File;

use crate::debug;
use crate::opus_mixer::audio_stream::AudioStream;
use crate::opus_mixer::{CHANNELS, FRAME_SIZE, SAMPLE_RATE};

/// Manages multiple audio streams and mixes their output
#[derive(Debug)]
pub struct AudioMixer {
    streams: Vec<AudioStream>,
    active_streams: usize,
    stream_finished: Vec<bool>,
    mixed_buffer: Vec<f32>,
    start_timestamp: f64,
    target_granule: i64,
    last_sync_check: i64,
    sync_interval: i64,
    max_sync_drift: f64, // Maximum observed drift between any two streams
}

impl AudioMixer {
    pub async fn new(files: Vec<File>, start_timestamp: f64) -> Result<Self, JsValue> {
        debug!("Creating mixer with {} streams", files.len());
        let stream_count = files.len();
        let target_granule = (start_timestamp * SAMPLE_RATE as f64) as i64;

        // Create streams asynchronously
        let mut streams = Vec::with_capacity(files.len());
        for file in files {
            streams.push(AudioStream::new(file).await?);
        }

        Ok(Self {
            streams,
            active_streams: stream_count,
            stream_finished: vec![false; stream_count],
            mixed_buffer: vec![0f32; FRAME_SIZE * CHANNELS as usize],
            start_timestamp,
            target_granule,
            last_sync_check: target_granule,
            sync_interval: SAMPLE_RATE as i64,
            max_sync_drift: 0.0,
        })
    }

    /// Check and adjust synchronization between streams
    fn check_sync(&mut self) {
        if self.target_granule - self.last_sync_check < self.sync_interval {
            return;
        }

        // Find the average position of active streams
        let mut total_pos = 0i64;
        let mut active_count = 0;
        let mut min_pos = i64::MAX;
        let mut max_pos = i64::MIN;

        for (idx, stream) in self.streams.iter().enumerate() {
            if !self.stream_finished[idx] {
                let pos = stream.current_granule_position;
                total_pos += pos;
                active_count += 1;
                min_pos = min_pos.min(pos);
                max_pos = max_pos.max(pos);
            }
        }

        if active_count > 1 {
            let avg_pos = total_pos / active_count;
            debug!("Average stream position: {}", avg_pos);

            // Update maximum observed drift
            let current_max_drift = (max_pos - min_pos) as f64 / SAMPLE_RATE as f64;
            self.max_sync_drift = self.max_sync_drift.max(current_max_drift);

            // Calculate and apply drift compensation
            for (idx, stream) in self.streams.iter_mut().enumerate() {
                if !self.stream_finished[idx] {
                    let drift = stream.current_granule_position as f64 - avg_pos as f64;
                    let drift_seconds = drift / SAMPLE_RATE as f64;

                    // Update drift statistics
                    stream.drift_stats.update_drift(drift_seconds);

                    // Adjust compensation based on drift
                    let max_adjustment = 0.02 * (self.target_granule - self.last_sync_check) as f64
                        / SAMPLE_RATE as f64;

                    if drift_seconds.abs() > 0.001 {
                        let adjustment = (drift_seconds.abs() / 1.0).min(max_adjustment);
                        stream.drift_compensation = if drift > 0.0 {
                            1.0 - adjustment as f32
                        } else {
                            1.0 + adjustment as f32
                        };
                        stream
                            .drift_stats
                            .update_compensation(stream.drift_compensation);
                        debug!(
                            "Stream {} drift: {:.3}s, compensation: {:.3}x",
                            idx, drift_seconds, stream.drift_compensation
                        );
                    } else {
                        stream.drift_compensation = 1.0;
                    }
                }
            }
        }

        self.last_sync_check = self.target_granule;
    }

    /// Seek to the desired timestamp in all streams using bisection search
    pub fn seek_to_timestamp(&mut self) -> Result<(), JsValue> {
        debug!(
            "Seeking all streams to timestamp: {:.2}s",
            self.start_timestamp
        );

        // Seek each stream to the target timestamp
        for (stream_idx, stream) in self.streams.iter_mut().enumerate() {
            debug!("Seeking stream {}", stream_idx);
            stream.seek_to_timestamp(self.start_timestamp)?;

            // Process headers after seeking
            while !stream.header_processed || !stream.comments_processed {
                match stream.process_next_packet()? {
                    Some(_) => {
                        debug!("Processed post-seek headers for stream {}", stream_idx);
                    }
                    None => {
                        if stream.header_processed && stream.comments_processed {
                            break;
                        }
                    }
                }
            }

            debug!(
                "Stream {} ready at timestamp {:.2}s",
                stream_idx,
                stream.current_timestamp()
            );
        }
        Ok(())
    }

    /// Mix the next batch of samples from all active streams
    pub fn mix_next_samples(&mut self) -> Result<Option<&[f32]>, JsValue> {
        if self.active_streams == 0 {
            debug!("No active streams remaining");
            return Ok(None);
        }

        self.mixed_buffer.fill(0.0);
        let mut samples_mixed = false;

        // Check and adjust synchronization
        self.check_sync();

        // Find the most behind stream that's not finished
        let min_granule = self
            .streams
            .iter()
            .enumerate()
            .filter(|(idx, _)| !self.stream_finished[*idx])
            .map(|(_, stream)| stream.current_granule_position)
            .min()
            .unwrap_or(self.target_granule);

        // Update target granule
        self.target_granule = min_granule;

        // Process each stream
        for (stream_idx, stream) in self.streams.iter_mut().enumerate() {
            if self.stream_finished[stream_idx] {
                continue;
            }

            // Check if this stream is ahead
            if stream.current_granule_position > self.target_granule + FRAME_SIZE as i64 {
                debug!(
                    "Stream {} is ahead (granule: {}, target: {}), skipping",
                    stream_idx, stream.current_granule_position, self.target_granule
                );
                continue;
            }

            debug!(
                "Processing stream {} at granule {}",
                stream_idx, stream.current_granule_position
            );
            match stream.process_next_packet()? {
                Some(decoded_samples) => {
                    debug!("Stream {} provided {} samples", stream_idx, decoded_samples);

                    let sample_count = decoded_samples * CHANNELS as usize;
                    let compensation = stream.drift_compensation;

                    // Apply drift compensation and mix into output buffer
                    for i in 0..sample_count {
                        self.mixed_buffer[i] += stream.get_decoded_samples()[i] * compensation
                            / self.active_streams as f32;
                    }
                    samples_mixed = true;
                }
                None => {
                    // Only mark the stream as finished if we've reached the end of the stream
                    // AND we've already processed both headers
                    if stream.header_processed && stream.comments_processed {
                        match stream
                            .packet_reader
                            .read_packet()
                            .map_err(|e| JsValue::from_str(&format!("Ogg read error: {}", e)))?
                        {
                            Some(_) => {
                                debug!("Stream {} waiting for more packets", stream_idx);
                            }
                            None => {
                                debug!("Stream {} reached end of file", stream_idx);
                                self.stream_finished[stream_idx] = true;
                                self.active_streams -= 1;
                                debug!("Active streams remaining: {}", self.active_streams);
                            }
                        }
                    } else {
                        debug!(
                            "Stream {} returned no samples (headers: {}/{}, decoder: {})",
                            stream_idx,
                            stream.header_processed,
                            stream.comments_processed,
                            stream.decoder.is_some()
                        );
                    }
                }
            }
        }

        // Update target granule position
        if samples_mixed {
            self.target_granule += FRAME_SIZE as i64;
        }

        if samples_mixed {
            Ok(Some(&self.mixed_buffer))
        } else {
            Ok(None)
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_streams > 0
    }

    /// Print detailed synchronization statistics
    pub fn print_sync_stats(&self) {
        println!("\nSynchronization Statistics:");
        println!("Maximum Sync Drift: {:.3} ms", self.max_sync_drift * 1000.0);

        for (idx, stream) in self.streams.iter().enumerate() {
            println!("\nStream {} Statistics:", idx);
            stream.drift_stats.print_stats();
        }
    }
}
