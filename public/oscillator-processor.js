// Audio worklet processor for reading from a shared buffer
class OscillatorProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();

    // Default values
    this.isInitialized = false;
    this.readPtr = 0;
    this.writePtr = 0;
    this.bufferSize = 0;
    this.metadataSize = 2; // [readPtr, writePtr]

    // Check if we have options with a shared buffer
    if (options && options.processorOptions && options.processorOptions.sharedBuffer) {
      // Get the shared buffer
      this.sharedBuffer = options.processorOptions.sharedBuffer;

      // Create a Float32Array view of the buffer
      this.bufferView = new Float32Array(this.sharedBuffer);

      // Get the buffer size (excluding metadata)
      this.bufferSize = this.bufferView.length - this.metadataSize;

      // Initialize
      this.isInitialized = true;

      console.log(`OscillatorProcessor initialized with buffer size: ${this.bufferSize}`);
    } else {
      console.error('OscillatorProcessor: No shared buffer provided');
    }
  }

  process(inputs, outputs) {
    // If not initialized, output silence
    if (!this.isInitialized) {
      return true;
    }

    const output = outputs[0];

    // Read the current write pointer from the shared buffer
    this.writePtr = this.bufferView[1];

    // Calculate available samples to read
    let available = 0;
    if (this.writePtr >= this.readPtr) {
      available = this.writePtr - this.readPtr;
    } else {
      available = this.bufferSize - this.readPtr + this.writePtr;
    }

    // Keep track of the last sample value for sample-and-hold during underruns
    if (!this.lastSample) {
      this.lastSample = 0;
    }

    // Fill all output channels
    for (let channel = 0; channel < output.length; channel++) {
      const outputChannel = output[channel];

      for (let i = 0; i < outputChannel.length; i++) {
        if (available > 0) {
          // Read a sample from the buffer
          const bufferIdx = (this.readPtr % this.bufferSize) + this.metadataSize;
          this.lastSample = this.bufferView[bufferIdx];
          outputChannel[i] = this.lastSample;

          // Increment the read pointer
          this.readPtr = (this.readPtr + 1) % this.bufferSize;
          available--;
        } else {
          // No more samples available, use sample-and-hold instead of silence
          // This prevents clicks and pops during buffer underruns
          outputChannel[i] = this.lastSample;

          // Log buffer underruns (but not too frequently to avoid console spam)
          if (Math.random() < 0.01) {
            console.warn('Buffer underrun detected in audio processor');
          }
        }
      }
    }

    // Update the read pointer in the shared buffer
    this.bufferView[0] = this.readPtr;

    // Return true to keep the processor running
    return true;
  }
}

try {
  console.log('OscillatorProcessor registering...');
  registerProcessor('oscillator-processor', OscillatorProcessor);
  console.log('OscillatorProcessor registered');
} catch (error) {
  console.error('Failed to register OscillatorProcessor:', error);
}
