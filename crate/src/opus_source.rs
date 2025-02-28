use crate::debug;
use crate::opus_mixer::audio_mixer::AudioMixer;
use crate::opus_mixer::{FRAME_SIZE, SAMPLE_RATE};
use crate::ring_buffer::RingBuffer;
use crate::source::Source;
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use wasm_bindgen::prelude::*;
use web_sys::File;

pub struct OpusSource {
    sample_rate: f32,
    ring_buffer: RingBuffer,
    mixer: Option<AudioMixer>,
    is_running: AtomicBool,
    file_loaded: bool,
}

impl OpusSource {
    pub fn new(sample_rate: f32) -> Result<Self, JsValue> {
        let ring_buffer = RingBuffer::new()?;

        Ok(Self {
            sample_rate,
            ring_buffer,
            mixer: None,
            is_running: AtomicBool::new(false),
            file_loaded: false,
        })
    }

    pub async fn load_file(&mut self, file: File) -> Result<(), JsValue> {
        // Create a vector with a single file
        let files = vec![file];

        // Create a new mixer with the file, starting at timestamp 0.0
        self.mixer = Some(AudioMixer::new(files, 0.0).await?);
        self.file_loaded = true;

        Ok(())
    }

    pub fn reset(&mut self) {
        if let Some(mixer) = &mut self.mixer {
            // Reset the mixer by seeking to the start timestamp
            let _ = mixer.seek_to_timestamp();
        }
    }

    pub fn is_file_loaded(&self) -> bool {
        self.file_loaded
    }
}

impl Source for OpusSource {
    fn get_ring_buffer(&self) -> RingBuffer {
        self.ring_buffer.clone()
    }

    fn start(&mut self) {
        self.is_running.store(true, Ordering::SeqCst);
    }

    fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    fn process(&mut self, num_samples: usize) -> usize {
        if !self.is_running.load(Ordering::SeqCst) || self.mixer.is_none() {
            return 0;
        }

        let mixer = self.mixer.as_mut().unwrap();

        // Update the read pointer based on what JavaScript has read
        self.ring_buffer.update_read_ptr();

        // TODO: get rid of num_samples, and just fill the buffer each time

        // Calculate how many frames we need to process
        // Note: For stereo, each frame contains twice as many samples as mono
        let available_samples = self.ring_buffer.available_write();
        let mut frames_to_process = (num_samples + FRAME_SIZE - 1) / FRAME_SIZE;
        let available_frames = available_samples / (FRAME_SIZE * 2); // Always 2 channels
        let mut total_samples_written = 0;

        debug!(
            "Processing {} frames, {} requested samples, {} available samples, {} available frames, 2 channels",
            frames_to_process, num_samples, available_samples, available_frames
        );

        if frames_to_process > available_frames {
            frames_to_process = available_frames;
        }

        for _ in 0..frames_to_process {
            // Mix the next frame of samples
            if let Ok(Some(mixed_samples)) = mixer.mix_next_samples() {
                // Write the mixed samples to the ring buffer
                let samples_written = self.ring_buffer.write(mixed_samples);
                total_samples_written += samples_written;

                // If we couldn't write all samples, the buffer is full
                if samples_written < mixed_samples.len() {
                    debug!(
                        "Dropped samples! {} samples written, {} samples needed",
                        samples_written,
                        mixed_samples.len()
                    );
                    break;
                }
            } else {
                // No more samples available or error occurred
                break;
            }
        }

        total_samples_written
    }

    fn get_shared_buffer(&self) -> js_sys::SharedArrayBuffer {
        self.ring_buffer.get_buffer()
    }

    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
