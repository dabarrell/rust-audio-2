// Web worker for handling oscillator processing
let oscillator;
let processorIntervalId;
let isInitialized = false;
let sharedBuffer;
let isInitializing = false;
let pendingOperations = [];

// Handle messages from the main thread
self.onmessage = async (event) => {
  console.log('Received message:', event.data);

  const { type, data } = event.data;

  switch (type) {
    case 'init':
      // Prevent multiple initialization attempts
      if (isInitializing || isInitialized) {
        console.log('Oscillator worker already initialized or initializing');

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
      await initWorker(data.sampleRate);
      break;

    case 'loadAudioFile':
      // Handle the audio file data
      if (data && data.file) {
        console.log('Received audio file in worker:', data.file.name);
        console.log('File type:', data.file.type);
        console.log('File size:', data.file.size, 'bytes');

        // Send acknowledgment back to main thread
        self.postMessage({
          type: 'audioFileReceived',
          success: true,
          fileName: data.file.name
        });
      } else {
        console.error('Invalid audio file data received');
        self.postMessage({
          type: 'audioFileReceived',
          success: false,
          error: 'Invalid audio file data'
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

      // Start the oscillator
      startOscillator();
      break;

    case 'stop':
      // Ignore if not initialized
      if (!isInitialized) {
        console.log('Ignoring stop operation - oscillator not initialized');
        return;
      }

      // Stop the oscillator
      stopOscillator();
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

    default:
      console.error('Unknown message type:', type);
  }
};

// Initialize the worker with the WASM module
async function initWorker(sampleRate) {
  try {
    // Import the WASM module
    // TODO: This re-downloads the wasm module. Explore passing the bytes from the main thread instead.
    const wasmImport = await import('/wasm/wasm_pack_test_27_feb.js');
    await wasmImport.default();

    const OscillatorClass = wasmImport.Oscillator;
    oscillator = new OscillatorClass(sampleRate);

    // Get the shared buffer to reuse later
    sharedBuffer = oscillator.get_shared_buffer();

    // Mark as initialized
    isInitialized = true;
    isInitializing = false;

    // Send back the shared buffer to the main thread
    self.postMessage({
      type: 'initialized',
      success: true,
      sharedBuffer
    });

    console.log('Oscillator worker initialized successfully');

    // Process any pending operations
    processPendingOperations();
  } catch (error) {
    console.error('Failed to initialize oscillator worker:', error);
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
          startOscillator();
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

// Start the oscillator
function startOscillator() {
  try {
    if (!isInitialized) {
      throw new Error('Oscillator not initialized');
    }

    if (!oscillator) {
      throw new Error('Oscillator missing');
    }

    // Start the oscillator
    oscillator.start();

    // Set up an interval to process audio samples
    if (processorIntervalId) {
      clearInterval(processorIntervalId);
    }

    processorIntervalId = setInterval(() => {
      if (oscillator) {
        // Process 256 samples at a time
        oscillator.process(256);
      }
    }, 2); // Process every 2ms

    // Send success message with the shared buffer
    self.postMessage({
      type: 'started',
      success: true,
      sharedBuffer
    });

    console.log('Oscillator started');
  } catch (error) {
    console.error('Failed to start oscillator:', error);
    self.postMessage({
      type: 'started',
      success: false,
      error: error.message
    });
  }
}

// Stop the oscillator
function stopOscillator() {
  try {
    if (!isInitialized || !oscillator) {
      throw new Error('Oscillator not initialized');
    }

    oscillator.stop();

    if (processorIntervalId) {
      clearInterval(processorIntervalId);
      processorIntervalId = null;
    }

    self.postMessage({
      type: 'stopped',
      success: true
    });

    console.log('Oscillator stopped');
  } catch (error) {
    console.error('Failed to stop oscillator:', error);
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
    if (!isInitialized || !oscillator) {
      throw new Error('Oscillator not initialized');
    }

    oscillator.set_frequency(frequency);

    self.postMessage({
      type: 'frequencySet',
      success: true
    });
  } catch (error) {
    console.error('Failed to set oscillator frequency:', error);
    self.postMessage({
      type: 'frequencySet',
      success: false,
      error: error.message
    });
  }
}
