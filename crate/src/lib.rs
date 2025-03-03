mod debug;
mod opus_mixer;
mod opus_source;
mod oscillator;
mod ring_buffer;
mod source;
mod utils;

use debug::set_debug;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioContext, AudioWorkletNode};

// Re-export the ring buffer and oscillator modules
pub use opus_source::OpusSource;
pub use oscillator::Oscillator;
pub use ring_buffer::{get_buffer_size, get_metadata_size, RingBuffer};
pub use source::{AudioSource, SourceType};

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct AudioEngineInterface {
    context: AudioContext,
    audio_output_node: Option<AudioWorkletNode>,
    shared_buffer: Option<js_sys::SharedArrayBuffer>,
    worker: Option<web_sys::Worker>,
    is_initialized: bool,
    pending_operations: Vec<PendingOperation>,
    audio_file_callback: Option<js_sys::Function>,
    source_type: String,
}

// Define an enum for pending operations
#[derive(Clone)]
enum PendingOperation {
    Start,
    SetFrequency(f32),
}

#[wasm_bindgen]
impl AudioEngineInterface {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<AudioEngineInterface, JsValue> {
        utils::set_panic_hook();
        set_debug(false);

        // Create a new audio context
        let context = AudioContext::new()?;

        Ok(AudioEngineInterface {
            context,
            audio_output_node: None,
            shared_buffer: None,
            worker: None,
            is_initialized: false,
            pending_operations: Vec::new(),
            audio_file_callback: None,
            source_type: "opusPlayer".to_string(), // Default to opusPlayer
        })
    }

    // Set the source type before initialization
    pub fn set_source_type(&mut self, source_type: &str) {
        self.source_type = source_type.to_string();
        log(&format!("Source type set to: {}", source_type));
    }

    pub async fn init(&mut self) -> Result<(), JsValue> {
        log("Initializing AudioEngineInterface");

        // Load the audio worklet processor
        let worklet = self.context.audio_worklet()?;
        let promise = worklet.add_module("/audio-output-processor.js")?;

        // Wait for the module to load
        JsFuture::from(promise).await?;

        log("Audio worklet module loaded successfully");

        // Create a web worker for the audio engine
        let worker = web_sys::Worker::new("/audio-engine-worker.js")?;

        // Store the worker
        self.worker = Some(worker.clone());

        // Set up a message handler for the worker that handles all message types
        let engine_ptr = self as *mut AudioEngineInterface;
        let context_clone = self.context.clone();
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
                "initialized" => {
                    if success {
                        log("Worker initialized successfully, setting up AudioWorkletNode");

                        // Get the shared buffer from the worker
                        if let Ok(buffer_val) =
                            js_sys::Reflect::get(&js_obj, &"sharedBuffer".into())
                        {
                            if !buffer_val.is_undefined() {
                                let shared_buffer = js_sys::SharedArrayBuffer::from(buffer_val);

                                // Create the audio output node with the shared buffer
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

                                if let Ok(audio_output_node) = AudioWorkletNode::new_with_options(
                                    &context_clone,
                                    "audio-output-processor",
                                    &options,
                                ) {
                                    // Connect the audio node to the audio output
                                    let _ = audio_output_node
                                        .connect_with_audio_node(&context_clone.destination());

                                    // Store the node in a global variable so it can be accessed later
                                    let window =
                                        web_sys::window().expect("no global window exists");
                                    js_sys::Reflect::set(
                                        &window,
                                        &"__audioOutputNode".into(),
                                        &audio_output_node,
                                    )
                                    .unwrap();

                                    log("AudioWorkletNode created and connected");

                                    // Update the engine state
                                    unsafe {
                                        if !engine_ptr.is_null() {
                                            let engine = &mut *engine_ptr;
                                            engine.audio_output_node = Some(audio_output_node);
                                            engine.shared_buffer = Some(shared_buffer);
                                            engine.is_initialized = true;

                                            // Process any pending operations
                                            let pending_ops =
                                                std::mem::take(&mut engine.pending_operations);
                                            for op in pending_ops {
                                                match op {
                                                    PendingOperation::Start => {
                                                        let _ = engine.start();
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
                    } else {
                        log("Failed to initialize worker");
                    }
                }
                "started" => {
                    if success {
                        log("Audio engine started successfully");
                    } else {
                        log("Failed to start audio engine");
                    }
                }
                "stopped" => {
                    if success {
                        log("Audio engine stopped successfully");
                    } else {
                        log("Failed to stop audio engine");
                    }
                }
                "frequencySet" => {
                    if success {
                        log("Frequency set successfully");
                    } else {
                        log("Failed to set frequency");
                    }
                }
                "audioFileReceived" => {
                    if success {
                        log("Audio file received by worker successfully");
                    } else {
                        log("Failed to process audio file in worker");
                    }

                    // Call the registered callback if available
                    unsafe {
                        if !engine_ptr.is_null() {
                            let engine = &mut *engine_ptr;
                            if let Some(callback) = &engine.audio_file_callback {
                                let _ = callback.call1(&JsValue::NULL, &js_obj);
                            }
                        }
                    }
                }
                _ => {
                    log(&format!("Unknown message type: {}", type_str));
                }
            }
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);

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

        // Pass the source type to the worker
        js_sys::Reflect::set(
            &init_data,
            &"sourceType".into(),
            &JsValue::from_str(&self.source_type),
        )?;

        js_sys::Reflect::set(&init_msg, &"data".into(), &init_data)?;

        worker.post_message(&init_msg)?;

        Ok(())
    }

    // Helper method to start the audio engine
    fn start(&self) -> Result<(), JsValue> {
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

    // Method to get the worker reference for direct communication
    pub fn get_worker(&self) -> Option<web_sys::Worker> {
        self.worker.clone()
    }

    // Method to send multiple audio files to the worker
    pub fn send_audio_files(&self, files: JsValue) -> Result<(), JsValue> {
        if !self.is_initialized {
            log("Cannot send audio files - engine not initialized");
            return Err(JsValue::from_str("Audio engine not initialized"));
        }

        if let Some(worker) = &self.worker {
            let msg = js_sys::Object::new();
            js_sys::Reflect::set(&msg, &"type".into(), &"loadAudioFiles".into())?;

            let data = js_sys::Object::new();
            js_sys::Reflect::set(&data, &"files".into(), &files)?;
            js_sys::Reflect::set(&msg, &"data".into(), &data)?;

            // Get file count if possible
            let file_count = if js_sys::Reflect::has(&files, &"length".into())? {
                js_sys::Reflect::get(&files, &"length".into())?
                    .as_f64()
                    .unwrap_or(0.0) as usize
            } else {
                0
            };

            log(&format!("Sending {} audio files to worker", file_count));
            worker.post_message(&msg)?;
        } else {
            return Err(JsValue::from_str("Worker not available"));
        }

        Ok(())
    }

    pub async fn resume(&mut self) -> Result<(), JsValue> {
        // Resume the audio context
        JsFuture::from(self.context.resume()?).await?;

        // Start the audio engine in the worker if initialized
        if !self.is_initialized {
            // Queue the operation for later
            self.pending_operations.push(PendingOperation::Start);
            log("Queuing start operation until initialization completes");
            return Ok(());
        }

        self.start()?;

        Ok(())
    }

    pub async fn suspend(&self) -> Result<(), JsValue> {
        // Only try to stop if initialized
        if self.is_initialized {
            // Stop the audio engine in the worker
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

    // Method to register a callback for audio file events
    pub fn set_audio_file_callback(&mut self, callback: js_sys::Function) {
        self.audio_file_callback = Some(callback);
        log("Audio file callback registered");
    }

    // Reset the audio source (for opus player)
    pub fn reset(&self) -> Result<(), JsValue> {
        if !self.is_initialized {
            return Err(JsValue::from_str("Audio engine not initialized"));
        }

        if let Some(worker) = &self.worker {
            let msg = js_sys::Object::new();
            js_sys::Reflect::set(&msg, &"type".into(), &"reset".into())?;
            worker.post_message(&msg)?;
        }

        Ok(())
    }

    // Get the current source type
    pub fn get_source_type(&self) -> String {
        self.source_type.clone()
    }
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, wasm-pack-test-27-feb!");
}
