// Audio worklet processor for reading from a shared buffer
class AudioOutputProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();

    // Default values
    this.isInitialized = false;
    this.readPtr = 0;
    this.writePtr = 0;
    this.bufferSize = 0;
    this.metadataSize = 2; // [readPtr, writePtr]
    this.channelCount = 2; // Always use 2 channels (stereo)

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

      console.log(`AudioOutputProcessor initialized with buffer size: ${this.bufferSize}, using stereo output`);
    } else {
      console.error('AudioOutputProcessor: No shared buffer provided');
    }
  }

  process(inputs, outputs) {
    // If not initialized, output silence
    if (!this.isInitialized) {
      return true;
    }

    const output = outputs[0];
    const outputChannels = output.length;

    // Read the current write pointer from the shared buffer
    this.writePtr = this.bufferView[1];

    // Calculate available samples to read (in frames, where each frame has 2 samples for stereo)
    let availableFrames = 0;
    if (this.writePtr >= this.readPtr) {
      availableFrames = Math.floor((this.writePtr - this.readPtr) / this.channelCount);
    } else {
      availableFrames = Math.floor((this.bufferSize - this.readPtr + this.writePtr) / this.channelCount);
    }

    // Keep track of the last sample values for sample-and-hold during underruns
    if (!this.lastSamples) {
      this.lastSamples = new Array(this.channelCount).fill(0);
    }

    // Get the length of the output buffer
    const outputLength = output[0].length;

    // Process each output sample
    for (let i = 0; i < outputLength; i++) {
      if (availableFrames > 0) {
        // For stereo, we need to read two consecutive samples for left and right channels
        for (let channel = 0; channel < Math.min(outputChannels, this.channelCount); channel++) {
          const bufferIdx = (this.readPtr % this.bufferSize) + this.metadataSize;
          this.lastSamples[channel] = this.bufferView[bufferIdx + channel];
          output[channel][i] = this.lastSamples[channel];
        }

        // Increment the read pointer by the number of channels (2 for stereo)
        this.readPtr = (this.readPtr + this.channelCount) % this.bufferSize;
        availableFrames--;
      } else {
        // No more samples available, use sample-and-hold instead of silence
        // This prevents clicks and pops during buffer underruns
        for (let channel = 0; channel < Math.min(outputChannels, this.channelCount); channel++) {
          output[channel][i] = this.lastSamples[channel];
        }

        // Log buffer underruns (but not too frequently to avoid console spam)
        if (Math.random() < 0.01) {
          console.warn('Buffer underrun detected in audio processor');
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
  console.log('AudioOutputProcessor registering...');
  registerProcessor('audio-output-processor', AudioOutputProcessor);
  console.log('AudioOutputProcessor registered');
} catch (error) {
  console.error('Failed to register AudioOutputProcessor:', error);
}
