use js_sys::{Float32Array, SharedArrayBuffer};
use std::sync::atomic::{AtomicUsize, Ordering};
use wasm_bindgen::prelude::*;

use crate::opus_mixer::FRAME_SIZE;

// Constants for the ring buffer
const BUFFER_SIZE: usize = FRAME_SIZE * 8; // Must be a power of 2
const BUFFER_MASK: usize = BUFFER_SIZE - 1; // For efficient modulo operations
const METADATA_SIZE: usize = 2; // For read and write pointers

#[wasm_bindgen]
pub struct RingBuffer {
    // The shared buffer that will be accessed by both Rust and JS
    buffer: SharedArrayBuffer,
    // Float32Array view of the buffer for easy access
    buffer_view: Float32Array,
    // Atomic read pointer (index where JS will read from)
    read_ptr: AtomicUsize,
    // Atomic write pointer (index where Rust will write to)
    write_ptr: AtomicUsize,

    // Metrics
    high_water_mark_read: AtomicUsize, // Maximum number of samples available to read
    high_water_mark_write: AtomicUsize, // Maximum number of samples available to write
    total_writes: AtomicUsize,         // Total number of write operations
    total_reads: AtomicUsize,          // Total number of read operations
    total_underruns: AtomicUsize,      // Total number of buffer underruns
    total_samples_written: AtomicUsize, // Total number of samples written
    total_samples_read: AtomicUsize,   // Total number of samples read
}

// Manual implementation of Clone for RingBuffer
impl Clone for RingBuffer {
    fn clone(&self) -> Self {
        RingBuffer {
            buffer: self.buffer.clone(),
            buffer_view: self.buffer_view.clone(),
            read_ptr: AtomicUsize::new(self.read_ptr.load(Ordering::Relaxed)),
            write_ptr: AtomicUsize::new(self.write_ptr.load(Ordering::Relaxed)),
            high_water_mark_read: AtomicUsize::new(
                self.high_water_mark_read.load(Ordering::Relaxed),
            ),
            high_water_mark_write: AtomicUsize::new(
                self.high_water_mark_write.load(Ordering::Relaxed),
            ),
            total_writes: AtomicUsize::new(self.total_writes.load(Ordering::Relaxed)),
            total_reads: AtomicUsize::new(self.total_reads.load(Ordering::Relaxed)),
            total_underruns: AtomicUsize::new(self.total_underruns.load(Ordering::Relaxed)),
            total_samples_written: AtomicUsize::new(
                self.total_samples_written.load(Ordering::Relaxed),
            ),
            total_samples_read: AtomicUsize::new(self.total_samples_read.load(Ordering::Relaxed)),
        }
    }
}

#[wasm_bindgen]
impl RingBuffer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<RingBuffer, JsValue> {
        // Create a SharedArrayBuffer with space for the audio data plus metadata
        // Metadata: [read_ptr, write_ptr, unused, unused]
        // Multiply by 4 because each float is 4 bytes
        let buffer = SharedArrayBuffer::new(((BUFFER_SIZE + METADATA_SIZE) * 4) as u32);
        let buffer_view = Float32Array::new(&buffer);

        // Initialize read and write pointers to 0
        buffer_view.set_index(0, 0.0); // read_ptr
        buffer_view.set_index(1, 0.0); // write_ptr

        Ok(RingBuffer {
            buffer,
            buffer_view,
            read_ptr: AtomicUsize::new(0),
            write_ptr: AtomicUsize::new(0),
            high_water_mark_read: AtomicUsize::new(0),
            high_water_mark_write: AtomicUsize::new(BUFFER_SIZE - 1), // Start with max available
            total_writes: AtomicUsize::new(0),
            total_reads: AtomicUsize::new(0),
            total_underruns: AtomicUsize::new(0),
            total_samples_written: AtomicUsize::new(0),
            total_samples_read: AtomicUsize::new(0),
        })
    }

    // Get the SharedArrayBuffer to pass to JavaScript
    pub fn get_buffer(&self) -> SharedArrayBuffer {
        self.buffer.clone()
    }

    // Write audio samples to the ring buffer
    pub fn write(&self, samples: &[f32]) -> usize {
        let write_ptr = self.write_ptr.load(Ordering::Acquire);
        let read_ptr = self.read_ptr.load(Ordering::Acquire);

        // Calculate available space, leaving one slot empty to distinguish full from empty
        let available = if write_ptr >= read_ptr {
            BUFFER_SIZE - (write_ptr - read_ptr) - 1
        } else {
            read_ptr - write_ptr - 1
        };

        // Don't write more than available space
        let to_write = samples.len().min(available);

        // Update metrics
        self.total_writes.fetch_add(1, Ordering::Relaxed);
        self.total_samples_written
            .fetch_add(to_write, Ordering::Relaxed);

        // Check if we couldn't write all samples (potential overrun)
        if to_write < samples.len() {
            // This is not an underrun but could be tracked as an overrun if needed
        }

        // Write samples to the buffer
        for i in 0..to_write {
            let buffer_idx = (write_ptr + i) & BUFFER_MASK;
            self.buffer_view
                .set_index((buffer_idx + METADATA_SIZE) as u32, samples[i]);
        }

        // Update write pointer atomically
        let new_write_ptr = (write_ptr + to_write) & BUFFER_MASK;
        self.write_ptr.store(new_write_ptr, Ordering::Release);

        // Update the write pointer in the shared buffer for JS to read
        self.buffer_view.set_index(1, new_write_ptr as f32);

        // Update high water mark for read availability
        let current_available_read = self.available_read();
        let current_high_water = self.high_water_mark_read.load(Ordering::Relaxed);
        if current_available_read > current_high_water {
            self.high_water_mark_read
                .store(current_available_read, Ordering::Relaxed);
        }

        // Update high water mark for write availability
        let current_available_write = self.available_write();
        let current_high_water = self.high_water_mark_write.load(Ordering::Relaxed);
        if current_available_write > current_high_water {
            self.high_water_mark_write
                .store(current_available_write, Ordering::Relaxed);
        }

        crate::debug!(
            "Writing {} samples (buffer: read={}, write={}, used={}%, hwm={}%)",
            to_write,
            read_ptr,
            write_ptr,
            (current_available_read as f32 / BUFFER_SIZE as f32) * 100.0,
            (self.high_water_mark_read.load(Ordering::Relaxed) as f32 / BUFFER_SIZE as f32) * 100.0
        );

        to_write
    }

    // Update the read pointer based on what JavaScript has read
    pub fn update_read_ptr(&self) {
        // Read the current read pointer from the shared buffer
        let js_read_ptr = self.buffer_view.get_index(0) as usize;
        let old_read_ptr = self.read_ptr.load(Ordering::Relaxed);

        // Calculate how many samples were read
        let samples_read = if js_read_ptr >= old_read_ptr {
            js_read_ptr - old_read_ptr
        } else {
            BUFFER_SIZE - old_read_ptr + js_read_ptr
        };

        // Update metrics
        if samples_read > 0 {
            self.total_reads.fetch_add(1, Ordering::Relaxed);
            self.total_samples_read
                .fetch_add(samples_read, Ordering::Relaxed);

            // Check for underruns - if JS tried to read more than was available
            let write_ptr = self.write_ptr.load(Ordering::Relaxed);
            let available_before_read = if write_ptr >= old_read_ptr {
                write_ptr - old_read_ptr
            } else {
                BUFFER_SIZE - old_read_ptr + write_ptr
            };

            if samples_read > available_before_read {
                self.total_underruns.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Update our local read pointer
        self.read_ptr.store(js_read_ptr, Ordering::Release);

        // Update high water marks after read
        let current_available_read = self.available_read();
        let current_high_water = self.high_water_mark_read.load(Ordering::Relaxed);
        if current_available_read > current_high_water {
            self.high_water_mark_read
                .store(current_available_read, Ordering::Relaxed);
        }

        let current_available_write = self.available_write();
        let current_high_water = self.high_water_mark_write.load(Ordering::Relaxed);
        if current_available_write > current_high_water {
            self.high_water_mark_write
                .store(current_available_write, Ordering::Relaxed);
        }
    }

    // Get the number of samples available to read
    pub fn available_read(&self) -> usize {
        let write_ptr = self.write_ptr.load(Ordering::Acquire);
        let read_ptr = self.read_ptr.load(Ordering::Acquire);

        if write_ptr >= read_ptr {
            write_ptr - read_ptr
        } else {
            BUFFER_SIZE - read_ptr + write_ptr
        }
    }

    // Get the number of samples that can be written
    pub fn available_write(&self) -> usize {
        let write_ptr = self.write_ptr.load(Ordering::Acquire);
        let read_ptr = self.read_ptr.load(Ordering::Acquire);

        // We need to leave one slot empty to distinguish between full and empty buffer
        if write_ptr >= read_ptr {
            BUFFER_SIZE - (write_ptr - read_ptr) - 1
        } else {
            read_ptr - write_ptr - 1
        }
    }

    // Get the buffer size (excluding metadata)
    pub fn get_buffer_size(&self) -> usize {
        BUFFER_SIZE
    }

    // Clear the buffer by resetting read and write pointers
    pub fn clear(&self) {
        // Reset both pointers to 0
        self.read_ptr.store(0, Ordering::Release);
        self.write_ptr.store(0, Ordering::Release);

        // Update the shared buffer
        self.buffer_view.set_index(0, 0.0); // read_ptr
        self.buffer_view.set_index(1, 0.0); // write_ptr

        // Note: We don't reset metrics here as they track lifetime statistics
    }

    // Methods to retrieve metrics

    // Get high water mark for read buffer (max samples available to read)
    pub fn get_high_water_mark_read(&self) -> usize {
        self.high_water_mark_read.load(Ordering::Relaxed)
    }

    // Get high water mark for write buffer (max samples available to write)
    pub fn get_high_water_mark_write(&self) -> usize {
        self.high_water_mark_write.load(Ordering::Relaxed)
    }

    // Get total number of write operations
    pub fn get_total_writes(&self) -> usize {
        self.total_writes.load(Ordering::Relaxed)
    }

    // Get total number of read operations
    pub fn get_total_reads(&self) -> usize {
        self.total_reads.load(Ordering::Relaxed)
    }

    // Get total number of buffer underruns
    pub fn get_total_underruns(&self) -> usize {
        self.total_underruns.load(Ordering::Relaxed)
    }

    // Get total number of samples written
    pub fn get_total_samples_written(&self) -> usize {
        self.total_samples_written.load(Ordering::Relaxed)
    }

    // Get total number of samples read
    pub fn get_total_samples_read(&self) -> usize {
        self.total_samples_read.load(Ordering::Relaxed)
    }

    // Reset all metrics
    pub fn reset_metrics(&self) {
        self.high_water_mark_read.store(0, Ordering::Relaxed);
        self.high_water_mark_write
            .store(BUFFER_SIZE - 1, Ordering::Relaxed);
        self.total_writes.store(0, Ordering::Relaxed);
        self.total_reads.store(0, Ordering::Relaxed);
        self.total_underruns.store(0, Ordering::Relaxed);
        self.total_samples_written.store(0, Ordering::Relaxed);
        self.total_samples_read.store(0, Ordering::Relaxed);
    }
}

// Constants exposed to JavaScript
#[wasm_bindgen]
pub fn get_buffer_size() -> usize {
    BUFFER_SIZE
}

#[wasm_bindgen]
pub fn get_metadata_size() -> usize {
    METADATA_SIZE
}
