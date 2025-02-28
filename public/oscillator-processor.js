// Audio worklet processor for reading from a shared buffer
class OscillatorProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();

    // Default values
    this.isInitialized = false;
    this.readPtr = 0;
    this.writePtr = 0;
    this.bufferSize = 0;
    this.metadataSize = 4; // [readPtr, writePtr, unused, unused]

    // Check if we have options with a shared buffer
    if (options && options.processorOptions && options.processorOptions.sharedBuffer) {
      // Get the shared buffer
      this.sharedBuffer = options.processorOptions.sharedBuffer;

      // Create a Float32Array view of the buffer
      this.bufferView = new Float32Array(this.sharedBuffer);

      // Get the buffer size (excluding metadata)
      this.bufferSize = this.bufferView.length - 4;

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

    // Fill all output channels
    for (let channel = 0; channel < output.length; channel++) {
      const outputChannel = output[channel];

      for (let i = 0; i < outputChannel.length; i++) {
        if (available > 0) {
          // Read a sample from the buffer
          const bufferIdx = (this.readPtr % this.bufferSize) + this.metadataSize;
          outputChannel[i] = this.bufferView[bufferIdx];

          // Increment the read pointer
          this.readPtr = (this.readPtr + 1) % this.bufferSize;
          available--;
        } else {
          // No more samples available, output silence
          outputChannel[i] = 0;
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
