'use client';

import React, { useCallback, useState } from 'react';

export interface UploadZoneProps {
  onFileReady?: (file: File) => void;
  onReset?: () => void;
  backendUrl?: string;
  enableBackendValidation?: boolean;
}

type UploadState = 'idle' | 'scanning' | 'success' | 'error';

export function UploadZone({ onFileReady, onReset }: UploadZoneProps) {
  const [uploadState, setUploadState] = useState<UploadState>('idle');
  const [errorMessage, setErrorMessage] = useState<string>('');

  const handleDrop = useCallback(
    async (files: FileList | File[] | null) => {
      const file = files?.[0];
      if (!file) {
        return;
      }

      if (!file.name.toLowerCase().endsWith('.wasm')) {
        setUploadState('error');
        setErrorMessage('Only .wasm files are supported.');
        return;
      }

      setUploadState('scanning');
      setErrorMessage('');

      try {
        setUploadState('success');
        onFileReady?.(file);
      } catch (error) {
        setUploadState('error');
        setErrorMessage(error instanceof Error ? error.message : 'The upload failed.');
      }
    },
    [onFileReady],
  );

  return (
    <div
      onDrop={(event) => {
        event.preventDefault();
        void handleDrop(event.dataTransfer.files);
      }}
      onDragOver={(event) => event.preventDefault()}
      style={{
        border: uploadState === 'error' ? '1px solid #fb8500' : '1px dashed #30363d',
        borderRadius: '12px',
        padding: '24px',
        backgroundColor: '#0d1117',
        color: '#c9d1d9',
        textAlign: 'center',
      }}
    >
      <input
        type="file"
        accept=".wasm"
        onChange={(event) => {
          void handleDrop(event.target.files);
        }}
        style={{ display: 'block', margin: '0 auto 12px' }}
      />
      <p style={{ margin: 0 }}>{uploadState === 'scanning' ? 'Scanning contract...' : uploadState === 'success' ? 'Contract ready.' : 'Drop a compiled Soroban contract (.wasm) here.'}</p>
      {errorMessage ? <p style={{ color: '#fb8500', marginTop: '8px' }}>{errorMessage}</p> : null}
      {onReset ? (
        <button type="button" onClick={onReset} style={{ marginTop: '12px' }}>
          Reset
        </button>
      ) : null}
    </div>
  );
}
