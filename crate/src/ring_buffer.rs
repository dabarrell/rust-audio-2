use js_sys::{Float32Array, SharedArrayBuffer};
use std::sync::atomic::{AtomicUsize, Ordering};
use wasm_bindgen::prelude::*;

// Constants for the ring buffer
const BUFFER_SIZE: usize = 4096; // Must be a power of 2
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
}

// Manual implementation of Clone for RingBuffer
impl Clone for RingBuffer {
    fn clone(&self) -> Self {
        RingBuffer {
            buffer: self.buffer.clone(),
            buffer_view: self.buffer_view.clone(),
            read_ptr: AtomicUsize::new(self.read_ptr.load(Ordering::Relaxed)),
            write_ptr: AtomicUsize::new(self.write_ptr.load(Ordering::Relaxed)),
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

        to_write
    }

    // Update the read pointer based on what JavaScript has read
    pub fn update_read_ptr(&self) {
        // Read the current read pointer from the shared buffer
        let js_read_ptr = self.buffer_view.get_index(0) as usize;

        // Update our local read pointer
        self.read_ptr.store(js_read_ptr, Ordering::Release);
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
