mod utils;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioContext, AudioWorkletNode};

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub struct AudioEngine {
    context: AudioContext,
    oscillator_node: Option<AudioWorkletNode>,
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
        })
    }

    pub async fn init(&mut self) -> Result<(), JsValue> {
        // Load the audio worklet processor
        let worklet = self.context.audio_worklet()?;
        let promise = worklet.add_module("/oscillator-processor.js")?;

        // Wait for the module to load
        JsFuture::from(promise).await?;

        log("Audio worklet module loaded successfully");

        // Create the oscillator node
        let oscillator_options = web_sys::AudioWorkletNodeOptions::new();
        let oscillator_node = AudioWorkletNode::new_with_options(
            &self.context,
            "oscillator-processor",
            &oscillator_options,
        )?;

        // Connect the oscillator to the audio output
        oscillator_node.connect_with_audio_node(&self.context.destination())?;

        self.oscillator_node = Some(oscillator_node);

        Ok(())
    }

    pub fn set_frequency(&self, frequency: f32) -> Result<(), JsValue> {
        if let Some(node) = &self.oscillator_node {
            let message = js_sys::Object::new();
            js_sys::Reflect::set(&message, &"type".into(), &"setFrequency".into())?;
            js_sys::Reflect::set(&message, &"frequency".into(), &frequency.into())?;

            let port = node.port()?;
            port.post_message(&message)?;
        }

        Ok(())
    }

    pub async fn resume(&self) -> Result<(), JsValue> {
        JsFuture::from(self.context.resume()?).await?;
        Ok(())
    }

    pub async fn suspend(&self) -> Result<(), JsValue> {
        JsFuture::from(self.context.suspend()?).await?;
        Ok(())
    }
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, wasm-pack-test-27-feb!");
}
