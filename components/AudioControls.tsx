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
  send_audio_files(files: FileList): Promise<void>;
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
  const [loadingDemoFiles, setLoadingDemoFiles] = useState(false);

  const demoFiles = [
    '/assets/git-it/bass.opus',
    '/assets/git-it/drums.opus',
    '/assets/git-it/keys.opus',
    '/assets/git-it/keys2.opus',
    '/assets/git-it/percussion.opus',
    '/assets/git-it/vox.opus'
  ];

  const arrayToFileList = (files: File[]): FileList => {
    const dataTransfer = new DataTransfer();
    files.forEach(file => dataTransfer.items.add(file));
    return dataTransfer.files;
  };

  const loadDemoFiles = async () => {
    if (!audioEngineRef.current) {
      setFileStatus('Error: Audio engine not initialized');
      return;
    }

    setLoadingDemoFiles(true);
    setFileStatus('Loading demo files...');

    try {
      const files = new Array<File>();

      for (const filePath of demoFiles) {
        const response = await fetch(filePath);
        const blob = await response.blob();
        const file = new File([blob], filePath.split('/').pop() || 'unknown', { type: 'audio/opus' });
        files.push(file);
      }

      // Convert array to FileList
      const fileList = arrayToFileList(files);

      await audioEngineRef.current.send_audio_files(fileList);
      setFileStatus('Demo files loaded successfully');
    } catch (err) {
      console.error('Error loading demo files:', err);
      setFileStatus(`Error: Failed to load demo files - ${err instanceof Error ? err.message : 'Unknown error'}`);
    } finally {
      setLoadingDemoFiles(false);
    }
  };

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
      const files = e.target.files;
      const fileCount = files.length;

      // Check if all files are audio files
      for (let i = 0; i < fileCount; i++) {
        if (!files[i].type.startsWith('audio/')) {
          setFileStatus('Error: Please select only audio files');
          return;
        }
      }

      const fileNames = Array.from(files).map(file => file.name).join(', ');
      setFileStatus(`${fileCount} file(s) selected: ${fileNames}. Sending to audio engine...`);

      // Automatically send the files to the audio engine
      if (audioEngineRef.current) {
        try {
          await audioEngineRef.current.send_audio_files(files);
          setFileStatus(`${fileCount} file(s) sent to audio engine`);
        } catch (err) {
          console.error('Error sending files to worker:', err);
          setFileStatus(`Error: ${err instanceof Error ? err.message : 'Failed to send files'}`);
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
                } text-white disabled:opacity-50 disabled:cursor-not-allowed mr-2`}
            >
              {isPlaying ? 'Pause' : 'Play'}
            </button>
            <button
              onClick={handleReset}
              disabled={!isInitialized}
              className="px-4 py-2 rounded bg-gray-500 hover:bg-gray-600 text-white disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Reset
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
            <div className="mb-4">
              <label className="block mb-2 font-medium">
                Audio Source:
              </label>
              <div className="space-y-4">
                <div className="p-4 border rounded-lg">
                  <h3 className="font-medium mb-2">Demo Files</h3>
                  <button
                    onClick={loadDemoFiles}
                    disabled={!isInitialized || loadingDemoFiles || sourceType !== 'opusPlayer'}
                    className="px-4 py-2 rounded bg-blue-500 hover:bg-blue-600 text-white disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {loadingDemoFiles ? 'Loading...' : 'Load Demo Files'}
                  </button>
                  <div className="mt-2 text-sm text-gray-600">
                    Loads a set of demo audio tracks (bass, drums, keys, percussion, vocals)
                  </div>
                </div>

                <div className="p-4 border rounded-lg">
                  <h3 className="font-medium mb-2">Upload Your Own Files</h3>
                  <input
                    type="file"
                    onChange={handleFileChange}
                    accept="audio/*"
                    multiple
                    disabled={!isInitialized || sourceType !== 'opusPlayer'}
                    className="block w-full text-sm text-gray-500
                      file:mr-4 file:py-2 file:px-4
                      file:rounded file:border-0
                      file:text-sm file:font-semibold
                      file:bg-blue-50 file:text-blue-700
                      hover:file:bg-blue-100
                      disabled:opacity-50 disabled:cursor-not-allowed"
                  />
                  <div className="mt-2 text-sm text-gray-600">
                    Select one or more audio files to upload
                  </div>
                </div>
              </div>
            </div>
          )}

          {fileStatus && (
            <div className="mb-4 p-3 bg-gray-50 rounded-lg">
              <p className="text-sm text-gray-700">{fileStatus}</p>
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
