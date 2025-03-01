// Web worker for handling audio engine processing
let audioSource;
let processorIntervalId;
let isInitialized = false;
let sharedBuffer;
let isInitializing = false;
let pendingOperations = [];
let sourceType = 'oscillator'; // Default source type

// TODO: Move most of this to rust

// Handle messages from the main thread
self.onmessage = async (event) => {
  console.log('Received message:', event.data);

  const { type, data } = event.data;

  switch (type) {
    case 'init':
      // Prevent multiple initialization attempts
      if (isInitializing || isInitialized) {
        console.log('Audio engine worker already initialized or initializing');

        // If already initialized, send the shared buffer again
        if (isInitialized && sharedBuffer) {
          self.postMessage({
            type: 'initialized',
            success: true,
            sharedBuffer
          });
        }
        return;
      }

      // Initialize the worker with the WASM module and shared buffer
      isInitializing = true;

      // Check if a source type was specified
      if (data && data.sourceType) {
        sourceType = data.sourceType;
      }

      await initWorker(data.sampleRate, sourceType);
      break;

    case 'loadAudioFile':
      // Handle the audio file data
      if (data && data.file) {
        console.log('Received audio file in worker:', data.file.name);
        console.log('File type:', data.file.type);
        console.log('File size:', data.file.size, 'bytes');

        try {
          // Load the file into the audio source
          if (audioSource && sourceType === 'opusPlayer') {
            await audioSource.loadAudioFile(data.file);

            // Send success message back to main thread
            self.postMessage({
              type: 'audioFileReceived',
              success: true,
              fileName: data.file.name
            });
          } else {
            throw new Error('Audio source not initialized or not an opus player');
          }
        } catch (error) {
          console.error('Error loading audio file:', error);

          // Send error message back to main thread
          self.postMessage({
            type: 'audioFileReceived',
            success: false,
            error: error.message || 'Failed to load audio file',
            fileName: data.file.name
          });
        }
      } else {
        console.error('Invalid audio file data received');
        self.postMessage({
          type: 'audioFileReceived',
          success: false,
          error: 'Invalid audio file data'
        });
      }
      break;

    case 'loadAudioFiles':
      // Handle multiple audio files
      if (data && data.files && data.files.length > 0) {
        const fileCount = data.files.length;
        console.log(`Received ${fileCount} audio files in worker`);

        try {
          // Convert FileList to array for easier handling
          const filesArray = Array.from(data.files);

          // Check if all files are valid
          for (const file of filesArray) {
            if (!file.type.startsWith('audio/')) {
              throw new Error(`File "${file.name}" is not an audio file`);
            }
          }

          // Load the files into the audio source
          if (audioSource && sourceType === 'opusPlayer') {
            await audioSource.loadAudioFiles(filesArray);

            // Get file names for the success message
            const fileNames = filesArray.map(file => file.name).join(', ');

            // Send success message back to main thread
            self.postMessage({
              type: 'audioFileReceived',
              success: true,
              fileName: `${fileCount} files: ${fileNames}`
            });
          } else {
            throw new Error('Audio source not initialized or not an opus player');
          }
        } catch (error) {
          console.error('Error loading audio files:', error);

          // Send error message back to main thread
          self.postMessage({
            type: 'audioFileReceived',
            success: false,
            error: error.message || 'Failed to load audio files'
          });
        }
      } else {
        console.error('Invalid audio files data received');
        self.postMessage({
          type: 'audioFileReceived',
          success: false,
          error: 'Invalid audio files data'
        });
      }
      break;

    case 'start':
      // Queue the operation if not initialized
      if (!isInitialized) {
        console.log('Queuing start operation until initialization completes');
        pendingOperations.push({ type: 'start' });
        return;
      }

      // Start the audio engine
      startAudioEngine();
      break;

    case 'stop':
      // Ignore if not initialized
      if (!isInitialized) {
        console.log('Ignoring stop operation - audio engine not initialized');
        return;
      }

      // Stop the audio engine
      stopAudioEngine();
      break;

    case 'setFrequency':
      // Queue the operation if not initialized
      if (!isInitialized) {
        console.log('Queuing setFrequency operation until initialization completes');
        pendingOperations.push({ type: 'setFrequency', frequency: data.frequency });
        return;
      }

      // Set the oscillator frequency
      setFrequency(data.frequency);
      break;

    case 'reset':
      // Reset the audio source (for opus player)
      if (!isInitialized) {
        console.log('Ignoring reset operation - audio engine not initialized');
        return;
      }

      resetAudioSource();
      break;

    default:
      console.error('Unknown message type:', type);
  }
};

// Initialize the worker with the WASM module
async function initWorker(sampleRate, sourceType = 'oscillator') {
  try {
    // Import the WASM module
    // TODO: This re-downloads the wasm module. Explore passing the bytes from the main thread instead.
    const wasmImport = await import('/wasm/wasm_pack_test_27_feb.js');
    await wasmImport.default();

    // Create the appropriate audio source based on the source type
    if (sourceType === 'oscillator') {
      audioSource = wasmImport.AudioSource.createOscillator(sampleRate);
    } else if (sourceType === 'opusPlayer') {
      audioSource = wasmImport.AudioSource.createOpusPlayer(sampleRate);
    } else {
      throw new Error(`Unknown source type: ${sourceType}`);
    }

    // Get the shared buffer to reuse later
    sharedBuffer = audioSource.get_shared_buffer();

    // Mark as initialized
    isInitialized = true;
    isInitializing = false;

    // Send back the shared buffer to the main thread
    self.postMessage({
      type: 'initialized',
      success: true,
      sharedBuffer,
      sourceType
    });

    console.log(`Audio engine worker initialized successfully with source type: ${sourceType}`);

    // Process any pending operations
    processPendingOperations();
  } catch (error) {
    console.error('Failed to initialize audio engine worker:', error);
    isInitializing = false;

    self.postMessage({
      type: 'initialized',
      success: false,
      error: error.message
    });
  }
}

// Process any operations that were queued during initialization
function processPendingOperations() {
  if (pendingOperations.length > 0) {
    console.log(`Processing ${pendingOperations.length} pending operations`);

    // Process each operation in order
    pendingOperations.forEach(op => {
      switch (op.type) {
        case 'start':
          startAudioEngine();
          break;
        case 'setFrequency':
          setFrequency(op.frequency);
          break;
      }
    });

    // Clear the queue
    pendingOperations = [];
  }
}

// Start the audio engine
function startAudioEngine() {
  try {
    if (!isInitialized) {
      throw new Error('Audio engine not initialized');
    }

    if (!audioSource) {
      throw new Error('Audio source missing');
    }

    // Start the audio source
    audioSource.start();

    // Set up an interval to process audio samples
    if (processorIntervalId) {
      clearInterval(processorIntervalId);
    }

    processorIntervalId = setInterval(() => {
      if (audioSource) {
        // Process FRAME_SIZE * 8 samples at a time
        audioSource.process(960 * 8);
      }
    }, 10); // Process every 2ms

    // Send success message with the shared buffer
    self.postMessage({
      type: 'started',
      success: true,
      sharedBuffer
    });

    console.log('Audio engine started');
  } catch (error) {
    console.error('Failed to start audio engine:', error);
    self.postMessage({
      type: 'started',
      success: false,
      error: error.message
    });
  }
}

// Stop the audio engine
function stopAudioEngine() {
  try {
    if (!isInitialized || !audioSource) {
      throw new Error('Audio engine not initialized');
    }

    audioSource.stop();

    if (processorIntervalId) {
      clearInterval(processorIntervalId);
      processorIntervalId = null;
    }

    self.postMessage({
      type: 'stopped',
      success: true
    });

    console.log('Audio engine stopped');
  } catch (error) {
    console.error('Failed to stop audio engine:', error);
    self.postMessage({
      type: 'stopped',
      success: false,
      error: error.message
    });
  }
}

// Set the oscillator frequency
function setFrequency(frequency) {
  try {
    if (!isInitialized || !audioSource) {
      throw new Error('Audio engine not initialized');
    }

    // Only set frequency if we're using an oscillator
    if (sourceType === 'oscillator') {
      audioSource.set_frequency(frequency);

      self.postMessage({
        type: 'frequencySet',
        success: true
      });
    } else {
      throw new Error('Cannot set frequency on non-oscillator source');
    }
  } catch (error) {
    console.error('Failed to set oscillator frequency:', error);
    self.postMessage({
      type: 'frequencySet',
      success: false,
      error: error.message
    });
  }
}

// Reset the audio source (for opus player)
function resetAudioSource() {
  try {
    if (!isInitialized || !audioSource) {
      throw new Error('Audio engine not initialized');
    }

    // Only reset if we're using an opus player
    if (sourceType === 'opusPlayer') {
      audioSource.reset();

      self.postMessage({
        type: 'reset',
        success: true
      });
    } else {
      throw new Error('Cannot reset non-opus player source');
    }
  } catch (error) {
    console.error('Failed to reset audio source:', error);
    self.postMessage({
      type: 'reset',
      success: false,
      error: error.message
    });
  }
}
