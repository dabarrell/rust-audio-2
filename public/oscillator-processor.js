// Audio worklet processor for a simple oscillator
class OscillatorProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.phase = 0;
    this.frequency = 440; // Default frequency in Hz

    // Handle messages from the main thread
    this.port.onmessage = (event) => {
      if (event.data.type === 'setFrequency') {
        this.frequency = event.data.frequency;
      }
    };
  }

  process(inputs, outputs, parameters) {
    const output = outputs[0];
    const sampleRate = 44100; // Standard sample rate

    // Use parameters if provided, otherwise use the instance property
    const frequency = parameters.frequency ? parameters.frequency[0] : this.frequency;

    // Calculate the phase increment per sample
    const phaseIncrement = 2 * Math.PI * frequency / sampleRate;

    // Fill all output channels with the oscillator signal
    for (let channel = 0; channel < output.length; channel++) {
      const outputChannel = output[channel];

      for (let i = 0; i < outputChannel.length; i++) {
        // Generate a sine wave
        outputChannel[i] = Math.sin(this.phase);

        // Increment the phase for the next sample
        this.phase += phaseIncrement;

        // Keep the phase in the range [0, 2π]
        if (this.phase > 2 * Math.PI) {
          this.phase -= 2 * Math.PI;
        }
      }
    }

    // Return true to keep the processor running
    return true;
  }
}

// Register the processor
registerProcessor('oscillator-processor', OscillatorProcessor);
