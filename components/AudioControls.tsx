'use client';

import { useEffect, useState, useRef } from 'react';

// We'll need to define the AudioEngine type since TypeScript doesn't know about it
interface AudioEngine {
  init(): Promise<void>;
  set_frequency(frequency: number): void;
  resume(): Promise<void>;
  suspend(): Promise<void>;
}

function AudioControls() {
  const [isPlaying, setIsPlaying] = useState(false);
  const [frequency, setFrequency] = useState(440);
  const audioEngineRef = useRef<AudioEngine | null>(null);
  const [isInitialized, setIsInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadWasm = async () => {
      try {
        setIsLoading(true);

        // Dynamically import the WASM module
        const wasmImport = await import('../public/wasm/wasm_pack_test_27_feb.js');
        await wasmImport.default();

        setIsLoading(false);

        // Create a new audio engine
        // Use type assertion to tell TypeScript about the AudioEngine constructor
        const AudioEngineClass = wasmImport.AudioEngine as unknown as { new(): AudioEngine };
        const engine = new AudioEngineClass();

        // Initialize the audio engine
        await engine.init();

        // Store the engine in the ref
        audioEngineRef.current = engine;
        setIsInitialized(true);
      } catch (err) {
        console.error('Failed to initialize audio engine:', err);
        setError('Failed to initialize audio engine. Check console for details.');
        setIsLoading(false);
      }
    };

    loadWasm();

    // Cleanup function
    return () => {
      if (audioEngineRef.current) {
        try {
          audioEngineRef.current.suspend().catch((err: Error) => {
            console.error('Error during cleanup:', err);
          });
        } catch (err) {
          console.error('Error during cleanup:', err);
        }
      }
    };
  }, []);

  const handlePlayPause = async () => {
    if (!audioEngineRef.current) return;

    try {
      if (isPlaying) {
        await audioEngineRef.current.suspend();
      } else {
        await audioEngineRef.current.resume();
      }
      setIsPlaying(!isPlaying);
    } catch (err) {
      console.error('Error toggling playback:', err);
      setError('Error toggling playback. Check console for details.');
    }
  };

  const handleFrequencyChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newFrequency = parseInt(e.target.value, 10);
    setFrequency(newFrequency);

    if (audioEngineRef.current) {
      try {
        audioEngineRef.current.set_frequency(newFrequency);
      } catch (err) {
        console.error('Error setting frequency:', err);
      }
    }
  };

  if (error) {
    return (
      <div className="p-4 bg-red-100 border border-red-400 text-red-700 rounded">
        <p>{error}</p>
      </div>
    );
  }

  return (
    <div className="p-4 border rounded-lg shadow-sm">
      <h2 className="text-xl font-bold mb-4">Rust WASM Oscillator</h2>

      {isLoading ? (
        <p className="text-gray-500">Loading WASM module...</p>
      ) : (
        <>
          <div className="mb-4">
            <button
              onClick={handlePlayPause}
              disabled={!isInitialized}
              className={`px-4 py-2 rounded ${isPlaying
                ? 'bg-red-500 hover:bg-red-600'
                : 'bg-green-500 hover:bg-green-600'
                } text-white disabled:opacity-50 disabled:cursor-not-allowed`}
            >
              {isPlaying ? 'Pause' : 'Play'}
            </button>
          </div>

          <div className="mb-4">
            <label className="block mb-2">
              Frequency: {frequency} Hz
            </label>
            <input
              type="range"
              min="20"
              max="2000"
              value={frequency}
              onChange={handleFrequencyChange}
              disabled={!isInitialized || !isPlaying}
              className="w-full"
            />
          </div>

          {!isInitialized && !isLoading && (
            <p className="text-gray-500">Initializing audio engine...</p>
          )}
        </>
      )}
    </div>
  );
}

export default AudioControls;
