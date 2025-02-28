mod oscillator;
mod ring_buffer;
mod utils;

use std::cell::RefCell;
use std::rc::Rc;
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
    oscillator: Option<Rc<RefCell<Oscillator>>>,
    processor_interval_id: Option<i32>,
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
            oscillator: None,
            processor_interval_id: None,
        })
    }

    pub async fn init(&mut self) -> Result<(), JsValue> {
        log("Once");
        // Load the audio worklet processor
        let worklet = self.context.audio_worklet()?;
        let promise = worklet.add_module("/oscillator-processor.js")?;

        // Wait for the module to load
        JsFuture::from(promise).await?;

        log("Audio worklet module loaded successfully");

        // Create the oscillator
        let sample_rate = self.context.sample_rate() as f32;
        let oscillator = Oscillator::new(sample_rate)?;

        // Get the shared buffer
        let shared_buffer = oscillator.get_shared_buffer();

        // Create the oscillator node with the shared buffer
        let options = web_sys::AudioWorkletNodeOptions::new();
        let processor_options = js_sys::Object::new();

        // Pass the shared buffer to the processor
        js_sys::Reflect::set(&processor_options, &"sharedBuffer".into(), &shared_buffer)?;
        options.set_processor_options(Some(&processor_options));

        let oscillator_node =
            AudioWorkletNode::new_with_options(&self.context, "oscillator-processor", &options)?;

        // Connect the oscillator to the audio output
        oscillator_node.connect_with_audio_node(&self.context.destination())?;

        // Store the oscillator and node
        let oscillator = Rc::new(RefCell::new(oscillator));
        self.oscillator = Some(oscillator.clone());
        self.oscillator_node = Some(oscillator_node);

        // Set up an interval to process audio samples
        let window = web_sys::window().expect("no global window exists");
        let oscillator_clone = oscillator.clone();

        // Create a closure that will be called periodically to process audio
        let closure = Closure::wrap(Box::new(move || {
            if let Ok(mut osc) = oscillator_clone.try_borrow_mut() {
                // Process 256 samples at a time (double the common audio buffer size)
                // to ensure we stay ahead of the audio worklet's consumption
                let _ = osc.process(256);
            }
        }) as Box<dyn FnMut()>);

        // Set up the interval (process every 2ms instead of 10ms)
        // This ensures we're generating samples faster than they're consumed
        let interval_id = window.set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            2,
        )?;

        // Forget the closure so it's not dropped
        closure.forget();

        self.processor_interval_id = Some(interval_id);

        Ok(())
    }

    pub fn set_frequency(&self, frequency: f32) -> Result<(), JsValue> {
        if let Some(oscillator) = &self.oscillator {
            if let Ok(mut osc) = oscillator.try_borrow_mut() {
                osc.set_frequency(frequency);
            }
        }

        Ok(())
    }

    pub async fn resume(&self) -> Result<(), JsValue> {
        // Resume the audio context
        JsFuture::from(self.context.resume()?).await?;

        // Start the oscillator
        if let Some(oscillator) = &self.oscillator {
            if let Ok(mut osc) = oscillator.try_borrow_mut() {
                osc.start();
            }
        }

        Ok(())
    }

    pub async fn suspend(&self) -> Result<(), JsValue> {
        // Stop the oscillator
        if let Some(oscillator) = &self.oscillator {
            if let Ok(mut osc) = oscillator.try_borrow_mut() {
                osc.stop();
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
