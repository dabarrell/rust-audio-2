'use client';

import { useEffect, useState, useRef } from 'react';

// Define the audio file event type
interface AudioFileEvent {
  type: string;
  success: boolean;
  fileName?: string;
  error?: string;
}

// We'll need to define the AudioEngineInterface type since TypeScript doesn't know about it
interface AudioEngineInterface {
  init(): Promise<void>;
  set_frequency(frequency: number): void;
  resume(): Promise<void>;
  suspend(): Promise<void>;
  send_audio_file(file: File): Promise<void>;
  set_audio_file_callback(callback: (event: AudioFileEvent) => void): void;
  set_source_type(sourceType: string): void;
  get_source_type(): string;
  reset(): Promise<void>;
}

// Define the available source types
type SourceType = 'oscillator' | 'opusPlayer';

function AudioControls() {
  const [isPlaying, setIsPlaying] = useState(false);
  const [frequency, setFrequency] = useState(440);
  const audioEngineRef = useRef<AudioEngineInterface | null>(null);
  const [isInitialized, setIsInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [fileStatus, setFileStatus] = useState<string | null>(null);
  const [sourceType, setSourceType] = useState<SourceType>('oscillator');
  const [needsReinitialization, setNeedsReinitialization] = useState(false);

  // TODO: This doesn't work with Strict Mode - fix it then re-enable

  useEffect(() => {
    const loadWasm = async () => {
      try {
        // setIsLoading(true);
        // Dynamically import the WASM module
        const wasmImport = await import('../public/wasm/wasm_pack_test_27_feb.js');
        await wasmImport.default();

        console.log('WASM module loaded');

        // Create a new audio engine
        // Use type assertion to tell TypeScript about the AudioEngineInterface constructor
        const AudioEngineInterfaceClass = wasmImport.AudioEngineInterface as unknown as { new(): AudioEngineInterface };
        const engine = new AudioEngineInterfaceClass();

        // Set the source type before initialization
        engine.set_source_type(sourceType);

        // Initialize the audio engine
        await engine.init();

        // Register the audio file callback
        engine.set_audio_file_callback((event) => {
          const { type, success, fileName, error } = event;

          if (type === 'audioFileReceived') {
            if (success) {
              setFileStatus(`File "${fileName}" successfully processed by worker`);
            } else {
              setFileStatus(`Error: ${error || 'Failed to process audio file'}`);
            }
          }
        });

        // Store the engine in the ref
        audioEngineRef.current = engine;
        setIsLoading(false);
        setIsInitialized(true);
      } catch (err) {
        console.error('Failed to initialize audio engine:', err);
        setError('Failed to initialize audio engine. Check console for details.');
        setIsLoading(false);
      }
    };

    if (!isInitialized && !isLoading) {
      setIsLoading(true);
      loadWasm();
    }

    // If source type changes, we need to reinitialize
    if (needsReinitialization && !isLoading) {
      // Clean up the old engine first
      if (audioEngineRef.current) {
        try {
          audioEngineRef.current.suspend().catch((err: Error) => {
            console.error('Error during cleanup:', err);
          });
        } catch (err) {
          console.error('Error during cleanup:', err);
        }
      }

      // Reset state
      setIsInitialized(false);
      setIsPlaying(false);
      setNeedsReinitialization(false);
      audioEngineRef.current = null;
    }

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
  }, [isInitialized, isLoading, sourceType, needsReinitialization]);

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

    if (audioEngineRef.current && sourceType === 'oscillator') {
      try {
        audioEngineRef.current.set_frequency(newFrequency);
      } catch (err) {
        console.error('Error setting frequency:', err);
      }
    }
  };

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      const file = e.target.files[0];

      // Check if it's an audio file
      if (!file.type.startsWith('audio/')) {
        setFileStatus('Error: Please select an audio file');
        return;
      }

      setFileStatus(`File "${file.name}" selected, sending to audio engine...`);

      // Automatically send the file to the audio engine
      if (audioEngineRef.current) {
        try {
          await audioEngineRef.current.send_audio_file(file);
          setFileStatus(`File "${file.name}" sent to audio engine`);
        } catch (err) {
          console.error('Error sending file to worker:', err);
          setFileStatus(`Error: ${err instanceof Error ? err.message : 'Failed to send file'}`);
        }
      } else {
        setFileStatus('Error: Audio engine not initialized');
      }
    }
  };

  const handleReset = async () => {
    if (!audioEngineRef.current) {
      return;
    }

    try {
      await audioEngineRef.current.reset();
      setFileStatus('Playback position reset to beginning');
    } catch (err) {
      console.error('Error resetting playback:', err);
      setFileStatus(`Error: ${err instanceof Error ? err.message : 'Failed to reset playback'}`);
    }
  };

  const handleSourceTypeChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const newSourceType = e.target.value as SourceType;
    setSourceType(newSourceType);
    setNeedsReinitialization(true);
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
      <h2 className="text-xl font-bold mb-4">Rust WASM Audio Engine</h2>

      {isLoading ? (
        <p className="text-gray-500">Loading WASM module...</p>
      ) : (
        <>
          <div className="mb-4">
            <label className="block mb-2 font-medium">
              Source Type:
            </label>
            <select
              value={sourceType}
              onChange={handleSourceTypeChange}
              disabled={isPlaying}
              className="w-full p-2 border rounded"
            >
              <option value="oscillator">Oscillator</option>
              <option value="opusPlayer">Opus Player</option>
            </select>
          </div>

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

          {sourceType === 'oscillator' && (
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
          )}

          {sourceType === 'opusPlayer' && (
            <div className="mb-4 mt-8 border-t pt-4">
              <h3 className="text-lg font-semibold mb-2">Audio File Loader</h3>
              <div className="mb-2">
                <label className="block mb-2 text-sm text-gray-700">
                  Select an audio file to play (file will be sent automatically):
                </label>
                <input
                  type="file"
                  accept="audio/*"
                  onChange={handleFileChange}
                  className="block w-full text-sm text-gray-500
                    file:mr-4 file:py-2 file:px-4
                    file:rounded file:border-0
                    file:text-sm file:font-semibold
                    file:bg-blue-50 file:text-blue-700
                    hover:file:bg-blue-100"
                />
              </div>

              <div className="mb-2">
                <button
                  onClick={handleReset}
                  disabled={!isInitialized}
                  className="px-4 py-2 rounded bg-gray-500 hover:bg-gray-600 text-white disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Reset Playback
                </button>
              </div>

              {fileStatus && (
                <div className={`mt-2 p-2 rounded text-sm ${fileStatus.startsWith('Error') ? 'bg-red-50 text-red-700' : 'bg-green-50 text-green-700'}`}>
                  {fileStatus}
                </div>
              )}
            </div>
          )}

          {!isInitialized && !isLoading && (
            <p className="text-gray-500">Initializing audio engine...</p>
          )}
        </>
      )}
    </div>
  );
}

export default AudioControls;
