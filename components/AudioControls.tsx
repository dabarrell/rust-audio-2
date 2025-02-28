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
}

function AudioControls() {
  const [isPlaying, setIsPlaying] = useState(false);
  const [frequency, setFrequency] = useState(440);
  const audioEngineRef = useRef<AudioEngineInterface | null>(null);
  const [isInitialized, setIsInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [fileStatus, setFileStatus] = useState<string | null>(null);

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
  }, [isInitialized, isLoading]);

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

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      const file = e.target.files[0];

      // Check if it's an audio file
      if (!file.type.startsWith('audio/')) {
        setFileStatus('Error: Please select an audio file');
        setSelectedFile(null);
        return;
      }

      setSelectedFile(file);
      setFileStatus(`File "${file.name}" selected`);
    }
  };

  const handleSendFile = async () => {
    if (!selectedFile || !audioEngineRef.current) {
      setFileStatus('Error: No file selected or audio engine not initialized');
      return;
    }

    try {
      // Send the file to the worker through the AudioEngine
      await audioEngineRef.current.send_audio_file(selectedFile);
      setFileStatus('Sending file to worker...');
    } catch (err) {
      console.error('Error sending file to worker:', err);
      setFileStatus(`Error: ${err instanceof Error ? err.message : 'Failed to send file'}`);
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
      <h2 className="text-xl font-bold mb-4">Rust WASM Audio Engine</h2>

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

          <div className="mb-4 mt-8 border-t pt-4">
            <h3 className="text-lg font-semibold mb-2">Audio File Loader</h3>
            <div className="mb-2">
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
                onClick={handleSendFile}
                disabled={!selectedFile || !isInitialized}
                className="px-4 py-2 rounded bg-blue-500 hover:bg-blue-600 text-white disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Send to Audio Engine
              </button>
            </div>

            {fileStatus && (
              <div className={`mt-2 p-2 rounded text-sm ${fileStatus.startsWith('Error') ? 'bg-red-50 text-red-700' : 'bg-green-50 text-green-700'}`}>
                {fileStatus}
              </div>
            )}
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
