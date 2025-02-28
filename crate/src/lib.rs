mod oscillator;
mod ring_buffer;
mod utils;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioContext, AudioWorkletNode};

// Re-export the ring buffer and oscillator modules
pub use oscillator::Oscillator;
pub use ring_buffer::{get_buffer_size, get_metadata_size, RingBuffer};

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct AudioEngine {
    context: AudioContext,
    oscillator_node: Option<AudioWorkletNode>,
    shared_buffer: Option<js_sys::SharedArrayBuffer>,
    worker: Option<web_sys::Worker>,
    is_initialized: bool,
    pending_operations: Vec<PendingOperation>,
}

// Define an enum for pending operations
#[derive(Clone)]
enum PendingOperation {
    Start,
    SetFrequency(f32),
}

#[wasm_bindgen]
impl AudioEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<AudioEngine, JsValue> {
        utils::set_panic_hook();

        // Create a new audio context
        let context = AudioContext::new()?;

        Ok(AudioEngine {
            context,
            oscillator_node: None,
            shared_buffer: None,
            worker: None,
            is_initialized: false,
            pending_operations: Vec::new(),
        })
    }

    pub async fn init(&mut self) -> Result<(), JsValue> {
        log("Initializing AudioEngine");

        // Load the audio worklet processor
        let worklet = self.context.audio_worklet()?;
        let promise = worklet.add_module("/oscillator-processor.js")?;

        // Wait for the module to load
        JsFuture::from(promise).await?;

        log("Audio worklet module loaded successfully");

        // Create a web worker for the oscillator
        let worker = web_sys::Worker::new("/oscillator-worker.js")?;

        // Set up message handler for the worker
        let callback = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
            let data = event.data();
            let js_obj = js_sys::Object::from(data);

            // Get the message type
            let type_val = js_sys::Reflect::get(&js_obj, &"type".into()).unwrap_or(JsValue::NULL);
            let type_str = type_val.as_string().unwrap_or_default();

            // Get the success flag
            let success_val =
                js_sys::Reflect::get(&js_obj, &"success".into()).unwrap_or(JsValue::NULL);
            let success = success_val.as_bool().unwrap_or(false);

            match type_str.as_str() {
                "started" => {
                    if success {
                        log("Oscillator started successfully");
                    } else {
                        log("Failed to start oscillator");
                    }
                }
                "stopped" => {
                    if success {
                        log("Oscillator stopped successfully");
                    } else {
                        log("Failed to stop oscillator");
                    }
                }
                "initialized" => {
                    if success {
                        log("Worker initialized successfully");
                    } else {
                        log("Failed to initialize worker");
                    }
                }
                "frequencySet" => {
                    if success {
                        log("Frequency set successfully");
                    } else {
                        log("Failed to set frequency");
                    }
                }
                _ => {
                    log(&format!("Unknown message type: {}", type_str));
                }
            }
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);

        worker.set_onmessage(Some(callback.as_ref().unchecked_ref()));
        callback.forget();

        // Store the worker
        self.worker = Some(worker.clone());

        // Set up a one-time message handler to get the shared buffer during initialization
        let engine_ptr = self as *mut AudioEngine;
        let context_clone = self.context.clone();
        let callback = Closure::once(Box::new(move |event: web_sys::MessageEvent| {
            let data = event.data();
            let js_obj = js_sys::Object::from(data);

            // Get the message type
            let type_val = js_sys::Reflect::get(&js_obj, &"type".into()).unwrap_or(JsValue::NULL);
            let type_str = type_val.as_string().unwrap_or_default();

            log(&format!("Received message type: {}", type_str));

            if type_str == "initialized" {
                // Get the success flag
                let success_val =
                    js_sys::Reflect::get(&js_obj, &"success".into()).unwrap_or(JsValue::NULL);
                let success = success_val.as_bool().unwrap_or(false);

                if success {
                    log("Worker initialized successfully, setting up AudioWorkletNode");

                    // Get the shared buffer from the worker
                    if let Ok(buffer_val) = js_sys::Reflect::get(&js_obj, &"sharedBuffer".into()) {
                        if !buffer_val.is_undefined() {
                            let shared_buffer = js_sys::SharedArrayBuffer::from(buffer_val);

                            // Create the oscillator node with the shared buffer
                            let options = web_sys::AudioWorkletNodeOptions::new();
                            let processor_options = js_sys::Object::new();

                            // Pass the shared buffer to the processor
                            js_sys::Reflect::set(
                                &processor_options,
                                &"sharedBuffer".into(),
                                &shared_buffer,
                            )
                            .unwrap();
                            options.set_processor_options(Some(&processor_options));

                            if let Ok(oscillator_node) = AudioWorkletNode::new_with_options(
                                &context_clone,
                                "oscillator-processor",
                                &options,
                            ) {
                                // Connect the oscillator to the audio output
                                let _ = oscillator_node
                                    .connect_with_audio_node(&context_clone.destination());

                                // Store the node in a global variable so it can be accessed later
                                let window = web_sys::window().expect("no global window exists");
                                js_sys::Reflect::set(
                                    &window,
                                    &"__oscillatorNode".into(),
                                    &oscillator_node,
                                )
                                .unwrap();

                                log("AudioWorkletNode created and connected");

                                // Update the engine state
                                unsafe {
                                    if !engine_ptr.is_null() {
                                        let engine = &mut *engine_ptr;
                                        engine.oscillator_node = Some(oscillator_node);
                                        engine.shared_buffer = Some(shared_buffer);
                                        engine.is_initialized = true;

                                        // Process any pending operations
                                        let pending_ops =
                                            std::mem::take(&mut engine.pending_operations);
                                        for op in pending_ops {
                                            match op {
                                                PendingOperation::Start => {
                                                    let _ = engine.start_oscillator();
                                                }
                                                PendingOperation::SetFrequency(freq) => {
                                                    let _ = engine.set_frequency(freq);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }) as Box<dyn FnOnce(web_sys::MessageEvent)>);

        worker.set_onmessage(Some(callback.as_ref().unchecked_ref()));
        callback.forget();

        // Initialize the worker with the WASM module URL and sample rate
        let init_msg = js_sys::Object::new();
        js_sys::Reflect::set(&init_msg, &"type".into(), &"init".into())?;

        let init_data = js_sys::Object::new();
        js_sys::Reflect::set(
            &init_data,
            &"sampleRate".into(),
            &JsValue::from_f64(self.context.sample_rate() as f64),
        )?;
        js_sys::Reflect::set(&init_msg, &"data".into(), &init_data)?;

        worker.post_message(&init_msg)?;

        Ok(())
    }

    // Helper method to start the oscillator
    fn start_oscillator(&self) -> Result<(), JsValue> {
        if let Some(worker) = &self.worker {
            let msg = js_sys::Object::new();
            js_sys::Reflect::set(&msg, &"type".into(), &"start".into())?;
            worker.post_message(&msg)?;
        }
        Ok(())
    }

    pub fn set_frequency(&mut self, frequency: f32) -> Result<(), JsValue> {
        if !self.is_initialized {
            // Queue the operation for later
            self.pending_operations
                .push(PendingOperation::SetFrequency(frequency));
            log("Queuing set_frequency operation until initialization completes");
            return Ok(());
        }

        if let Some(worker) = &self.worker {
            let msg = js_sys::Object::new();
            js_sys::Reflect::set(&msg, &"type".into(), &"setFrequency".into())?;

            let data = js_sys::Object::new();
            js_sys::Reflect::set(
                &data,
                &"frequency".into(),
                &JsValue::from_f64(frequency as f64),
            )?;
            js_sys::Reflect::set(&msg, &"data".into(), &data)?;

            worker.post_message(&msg)?;
        }

        Ok(())
    }

    pub async fn resume(&mut self) -> Result<(), JsValue> {
        // Resume the audio context
        JsFuture::from(self.context.resume()?).await?;

        // Start the oscillator in the worker if initialized
        if !self.is_initialized {
            // Queue the operation for later
            self.pending_operations.push(PendingOperation::Start);
            log("Queuing start operation until initialization completes");
            return Ok(());
        }

        self.start_oscillator()?;

        Ok(())
    }

    pub async fn suspend(&self) -> Result<(), JsValue> {
        // Only try to stop if initialized
        if self.is_initialized {
            // Stop the oscillator in the worker
            if let Some(worker) = &self.worker {
                let msg = js_sys::Object::new();
                js_sys::Reflect::set(&msg, &"type".into(), &"stop".into())?;
                worker.post_message(&msg)?;
            }
        }

        // Suspend the audio context
        JsFuture::from(self.context.suspend()?).await?;

        Ok(())
    }
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, wasm-pack-test-27-feb!");
}
