use crate::ring_buffer::RingBuffer;
use std::any::Any;
use wasm_bindgen::prelude::*;

// Source trait defines the common interface for all audio sources
pub trait Source {
    // Get the ring buffer to pass to JavaScript
    fn get_ring_buffer(&self) -> RingBuffer;

    // Start the source
    fn start(&mut self);

    // Stop the source
    fn stop(&mut self);

    // Generate audio samples and write them to the ring buffer
    fn process(&mut self, num_samples: usize) -> usize;

    // Get the shared buffer to pass to JavaScript
    fn get_shared_buffer(&self) -> js_sys::SharedArrayBuffer;

    // Check if the source is running
    fn is_running(&self) -> bool;

    // Required for downcasting
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

// SourceType enum to identify different types of sources
#[wasm_bindgen]
#[derive(Clone)]
pub enum SourceType {
    Oscillator,
    OpusPlayer,
    // Add more source types here as they are implemented
    // Example: SamplePlayer,
    // Example: NoiseGenerator,
}

// A wrapper struct that will be exposed to JavaScript
#[wasm_bindgen]
pub struct AudioSource {
    // The actual source implementation
    source_type: SourceType,
    source: Box<dyn Source>,
}

#[wasm_bindgen]
impl AudioSource {
    // Create a new oscillator source
    #[wasm_bindgen(js_name = createOscillator)]
    pub fn create_oscillator(sample_rate: f32) -> Result<AudioSource, JsValue> {
        use crate::oscillator::Oscillator;

        let oscillator = Oscillator::new(sample_rate)?;

        Ok(AudioSource {
            source_type: SourceType::Oscillator,
            source: Box::new(oscillator),
        })
    }

    // Create a new opus player source
    #[wasm_bindgen(js_name = createOpusPlayer)]
    pub fn create_opus_player(sample_rate: f32) -> Result<AudioSource, JsValue> {
        use crate::opus_source::OpusSource;

        let opus_source = OpusSource::new(sample_rate)?;

        Ok(AudioSource {
            source_type: SourceType::OpusPlayer,
            source: Box::new(opus_source),
        })
    }

    // Get the type of this source
    pub fn get_type(&self) -> SourceType {
        self.source_type.clone()
    }

    // Start the source
    pub fn start(&mut self) {
        self.source.start();
    }

    // Stop the source
    pub fn stop(&mut self) {
        self.source.stop();
    }

    // Process audio samples
    pub fn process(&mut self, num_samples: usize) -> usize {
        self.source.process(num_samples)
    }

    // Get the shared buffer
    pub fn get_shared_buffer(&self) -> js_sys::SharedArrayBuffer {
        self.source.get_shared_buffer()
    }

    // Check if the source is running
    pub fn is_running(&self) -> bool {
        self.source.is_running()
    }

    // Set frequency (only for oscillator type)
    pub fn set_frequency(&mut self, frequency: f32) -> Result<(), JsValue> {
        match self.source_type {
            SourceType::Oscillator => {
                // Downcast to Oscillator and set frequency
                if let Some(oscillator) = self
                    .source
                    .as_mut()
                    .as_any_mut()
                    .downcast_mut::<crate::oscillator::Oscillator>()
                {
                    oscillator.set_frequency(frequency);
                    Ok(())
                } else {
                    Err(JsValue::from_str("Failed to downcast to Oscillator"))
                }
            }
            // Add more source types here as they are implemented
            _ => Err(JsValue::from_str(
                "This source type does not support set_frequency",
            )),
        }
    }

    // Load an audio file (only for opus player type)
    #[wasm_bindgen(js_name = loadAudioFile)]
    pub async fn load_audio_file(&mut self, file: web_sys::File) -> Result<(), JsValue> {
        match self.source_type {
            SourceType::OpusPlayer => {
                // Downcast to OpusSource and load the file
                if let Some(opus_source) = self
                    .source
                    .as_mut()
                    .as_any_mut()
                    .downcast_mut::<crate::opus_source::OpusSource>()
                {
                    opus_source.load_file(file).await
                } else {
                    Err(JsValue::from_str("Failed to downcast to OpusSource"))
                }
            }
            // Add more source types here as they are implemented
            _ => Err(JsValue::from_str(
                "This source type does not support loading audio files",
            )),
        }
    }

    // Load multiple audio files (only for opus player type)
    #[wasm_bindgen(js_name = loadAudioFiles)]
    pub async fn load_audio_files(&mut self, files_js: js_sys::Array) -> Result<(), JsValue> {
        match self.source_type {
            SourceType::OpusPlayer => {
                // Downcast to OpusSource
                if let Some(opus_source) = self
                    .source
                    .as_mut()
                    .as_any_mut()
                    .downcast_mut::<crate::opus_source::OpusSource>()
                {
                    // Convert JS array to Rust Vec<File>
                    let mut files = Vec::with_capacity(files_js.length() as usize);
                    for i in 0..files_js.length() {
                        let file_js = files_js.get(i);
                        let file: web_sys::File = file_js.dyn_into()?;
                        files.push(file);
                    }

                    // Load the files
                    opus_source.load_files(files).await
                } else {
                    Err(JsValue::from_str("Failed to downcast to OpusSource"))
                }
            }
            // Add more source types here as they are implemented
            _ => Err(JsValue::from_str(
                "This source type does not support loading audio files",
            )),
        }
    }

    // Reset playback position (only for opus player type)
    pub fn reset(&mut self) -> Result<(), JsValue> {
        match self.source_type {
            SourceType::OpusPlayer => {
                // Downcast to OpusSource and reset
                if let Some(opus_source) = self
                    .source
                    .as_mut()
                    .as_any_mut()
                    .downcast_mut::<crate::opus_source::OpusSource>()
                {
                    opus_source.reset();
                    Ok(())
                } else {
                    Err(JsValue::from_str("Failed to downcast to OpusSource"))
                }
            }
            // Add more source types here as they are implemented
            _ => Err(JsValue::from_str("This source type does not support reset")),
        }
    }

    // Check if a file is loaded (only for opus player type)
    pub fn is_file_loaded(&self) -> Result<bool, JsValue> {
        match self.source_type {
            SourceType::OpusPlayer => {
                // Downcast to OpusSource and check if file is loaded
                if let Some(opus_source) = self
                    .source
                    .as_any()
                    .downcast_ref::<crate::opus_source::OpusSource>()
                {
                    Ok(opus_source.is_file_loaded())
                } else {
                    Err(JsValue::from_str("Failed to downcast to OpusSource"))
                }
            }
            // Add more source types here as they are implemented
            _ => Err(JsValue::from_str(
                "This source type does not support is_file_loaded",
            )),
        }
    }
}
