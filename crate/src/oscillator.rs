use crate::ring_buffer::RingBuffer;
use crate::source::Source;
use libm::sinf;
use std::any::Any;
use wasm_bindgen::prelude::JsValue;

pub struct Oscillator {
    // The ring buffer to write audio samples to
    ring_buffer: RingBuffer,
    // Current phase of the oscillator
    phase: f32,
    // Frequency in Hz
    frequency: f32,
    // Sample rate in Hz
    sample_rate: f32,
    // Whether the oscillator is running
    is_running: bool,
}

// Manual implementation of Clone for Oscillator
impl Clone for Oscillator {
    fn clone(&self) -> Self {
        Oscillator {
            ring_buffer: self.ring_buffer.clone(),
            phase: self.phase,
            frequency: self.frequency,
            sample_rate: self.sample_rate,
            is_running: self.is_running,
        }
    }
}

// Implement the Source trait for Oscillator
impl Source for Oscillator {
    fn get_ring_buffer(&self) -> RingBuffer {
        self.ring_buffer.clone()
    }

    fn start(&mut self) {
        self.is_running = true;
    }

    fn stop(&mut self) {
        self.is_running = false;
    }

    fn process(&mut self, num_samples: usize) -> usize {
        // Update the read pointer from JavaScript
        self.ring_buffer.update_read_ptr();

        // If not running, don't generate any samples
        if !self.is_running {
            return 0;
        }

        // Calculate how many samples we can write
        let available = self.ring_buffer.available_write();
        let to_process = num_samples.min(available);

        // Generate samples
        let mut samples = vec![0.0; to_process];

        // Calculate the phase increment per sample
        let phase_increment = 2.0 * std::f32::consts::PI * self.frequency / self.sample_rate;

        // Generate sine wave samples
        for i in 0..to_process {
            // Generate a sine wave using libm's sinf (safe wrapper)
            samples[i] = sinf(self.phase);

            // Increment the phase for the next sample
            self.phase += phase_increment;

            // Keep the phase in the range [0, 2π]
            if self.phase > 2.0 * std::f32::consts::PI {
                self.phase -= 2.0 * std::f32::consts::PI;
            }
        }

        // Write the samples to the ring buffer
        self.ring_buffer.write(&samples)
    }

    fn get_shared_buffer(&self) -> js_sys::SharedArrayBuffer {
        self.ring_buffer.get_buffer()
    }

    fn is_running(&self) -> bool {
        self.is_running
    }

    // Required for downcasting
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Oscillator {
    pub fn new(sample_rate: f32) -> Result<Oscillator, JsValue> {
        let ring_buffer = RingBuffer::new()?;

        Ok(Oscillator {
            ring_buffer,
            phase: 0.0,
            frequency: 440.0, // Default to A4
            sample_rate,
            is_running: false,
        })
    }

    // Set the frequency of the oscillator
    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
    }
}
