'use client';

import React, { useCallback, useState } from 'react';
import { useDropzone, FileRejection } from 'react-dropzone';
import { parseWasmError } from '../lib/errorHandling';
import { arrayBufferToBase64 } from '../lib/utils';
import { getApiBaseUrl } from '../lib/api';
import { useWasmValidationWorker } from '../hooks/useWasmValidationWorker';
import {
  isWithinMaxFileSize,
  MAX_WASM_FILE_SIZE_BYTES,
  readFileInChunks,
  validateWasmBuffer,
} from '../lib/wasmUpload';

// ─── Constants ────────────────────────────────────────────────────────────────

const MAX_WASM_SIZE = 10 * 1024 * 1024; // 10 MB limit

// Helper to check for WASM magic header (\0asm)
function hasWasmMagic(buffer: ArrayBuffer): boolean {
  if (buffer.byteLength < 4) return false;
  const view = new DataView(buffer);
  return view.getUint32(0, false) === 0x0061736d;
}

// ─── Types ────────────────────────────────────────────────────────────────────

type UploadState = 'idle' | 'hover' | 'scanning' | 'submitting' | 'success' | 'error';

interface DroppedFile {
  name: string;
  sizeBytes: number;
}

interface ErrorDetails {
  title: string;
  message: string;
  details?: string;
  suggestedAction?: string;
}

export interface UploadZoneProps {
  /** Called with the validated File once scanning completes */
  onFileReady?: (file: File) => void;
  /** Backend endpoint for WASM analysis (default: http://localhost:8080/analyze/wasm) */
  backendUrl?: string;
  /** Whether to validate with backend after client-side checks */
  enableBackendValidation?: boolean;
  /** Called when user resets the upload */
  onReset?: () => void;
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function WasmIcon({ state }: { state: UploadState }) {
  const isActive = state === 'hover' || state === 'scanning' || state === 'submitting' || state === 'success';
  return (
    <svg
      width="64"
      height="64"
      viewBox="0 0 64 64"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={`transition-all duration-500 ${isActive ? 'scale-110' : 'scale-100'}`}
    >
      <path
        d="M32 4 L56 18 L56 46 L32 60 L8 46 L8 18 Z"
        stroke={
          state === 'error'
            ? '#f87171'
            : state === 'success'
            ? '#34d399'
            : state === 'scanning' || state === 'submitting'
            ? '#a78bfa'
            : state === 'hover'
            ? '#38bdf8'
            : '#334155'
        }
        strokeWidth="2"
        fill={
          state === 'error'
            ? 'rgba(248,113,113,0.08)'
            : state === 'success'
            ? 'rgba(52,211,153,0.08)'
            : state === 'scanning' || state === 'submitting'
            ? 'rgba(167,139,250,0.08)'
            : state === 'hover'
            ? 'rgba(56,189,248,0.08)'
            : 'rgba(30,41,59,0.5)'
        }
        className="transition-all duration-500"
      />
      <text
        x="32"
        y="35"
        textAnchor="middle"
        fontSize="11"
        fontWeight="700"
        fontFamily="monospace"
        fill={
          state === 'error'
            ? '#f87171'
            : state === 'success'
            ? '#34d399'
            : state === 'scanning' || state === 'submitting'
            ? '#a78bfa'
            : state === 'hover'
            ? '#38bdf8'
            : '#64748b'
        }
        className="transition-all duration-500"
      >
        .wasm
      </text>
    </svg>
  );
}

function UploadProgressBar({ progress, label = 'Reading file...' }: { progress: number; label?: string }) {
  return (
    <div className="w-full mt-3">
      <div className="flex justify-between text-xs text-slate-400 mb-1">
        <span>{label}</span>
        <span>{progress}%</span>
      </div>
      <div className="w-full overflow-hidden rounded-full h-2 bg-slate-800">
        <div
          className="h-full rounded-full bg-gradient-to-r from-sky-500 to-cyan-400 transition-all duration-300 ease-out"
          style={{ width: `${progress}%` }}
        />
      </div>
    </div>
  );
}

function ScanningAnimation() {
  return (
    <div className="w-full mt-3 overflow-hidden rounded-full h-1 bg-slate-800">
      <div
        className="h-full rounded-full bg-gradient-to-r from-violet-500 via-fuchsia-400 to-violet-500"
        style={{
          animation: 'scan-sweep 1.6s ease-in-out infinite',
          backgroundSize: '200% 100%',
        }}
      />
      <style>{`
        @keyframes scan-sweep {
          0%   { transform: translateX(-100%); }
          100% { transform: translateX(200%); }
        }
      `}</style>
    </div>
  );
}

function SpinnerDots() {
  return (
    <div className="flex gap-1.5 items-center justify-center mt-2">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="w-1.5 h-1.5 rounded-full bg-violet-400"
          style={{
            animation: `dot-pulse 1.2s ease-in-out ${i * 0.2}s infinite`,
          }}
        />
      ))}
      <style>{`
        @keyframes dot-pulse {
          0%, 80%, 100% { opacity: 0.2; transform: scale(0.8); }
          40%            { opacity: 1;   transform: scale(1.2); }
        }
      `}</style>
    </div>
  );
}

function SuccessIcon() {
  return (
    <svg
      className="w-5 h-5 text-emerald-400 inline-block mr-1.5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth={2.5}
    >
      <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
    </svg>
  );
}

function ErrorIcon() {
  return (
    <svg
      className="w-5 h-5 text-red-400 inline-block mr-1.5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth={2.5}
    >
      <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
    </svg>
  );
}

export interface UploadZoneProps {
  onFileReady?: (file: File) => void;
  backendUrl?: string;
  enableBackendValidation?: boolean;
  onReset?: () => void;
}
// ─── Main Component ───────────────────────────────────────────────────────────

export function UploadZone({
  onFileReady,
  onReset,
  backendUrl,
  enableBackendValidation = true
  backendUrl = 'http://localhost:8080/analyze/wasm',
  enableBackendValidation = true,
}: UploadZoneProps) {
  // Falls back to the configured backend (see /settings) when not overridden.
  const analyzeUrl = backendUrl ?? `${getApiBaseUrl()}/analyze/wasm`;
  const { validate } = useWasmValidationWorker();
  const [uploadState, setUploadState] = useState<UploadState>('idle');
  const [droppedFile, setDroppedFile] = useState<DroppedFile | null>(null);
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [, setErrorDetails] = useState<ErrorDetails | null>(null);
  const [unexpectedError, setUnexpectedError] = useState<Error | null>(null);
  const [uploadProgress, setUploadProgress] = useState(0);

  if (unexpectedError) {
    throw unexpectedError;
  }

  const submitToBackend = useCallback(async (file: File, arrayBuffer: ArrayBuffer): Promise<boolean> => {
    try {
      setUploadState('submitting');

      const base64Data = arrayBufferToBase64(arrayBuffer);

      return new Promise((resolve) => {
        reader.onload = async (event) => {
            setUploadProgress(100);
            const arrayBuffer = event.target?.result as ArrayBuffer;
            if (!arrayBuffer) throw new Error('Failed to read file');
      const response = await fetch(backendUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          wasm_bytes: base64Data,
          function_name: 'main',
          args: [],
        }),
      });

      if (!response.ok) {
        const contentType = response.headers.get('content-type');
        if (contentType && contentType.includes('application/json')) {
          const errData = await response.json();

            const response = await fetch(analyzeUrl, {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                wasm_bytes: base64Data,
                function_name: 'main',
                args: [],
              }),
            });

            if (!response.ok) {
              const errorText = await response.text();
              const contentType = response.headers.get('content-type');
              if (contentType && contentType.includes('application/json')) {
                const errData = await response.json();

                if (errData.error && typeof errData.error === 'object') {
                  const backendMessage =
                    typeof errData.error.message === 'string'
                      ? errData.error.message
                      : errorText;
                  const parseResult = parseWasmError(response, backendMessage);

                  setErrorDetails({
                    title: parseResult.title,
                    message: parseResult.message,
                    details: parseResult.details,
                    suggestedAction: parseResult.suggestedAction
                  setErrorMessage(parseResult.message);
  // ── Backend submission ───────────────────────────────────────────────────────

  const submitToBackend = useCallback(
    async (file: File): Promise<boolean> => {

        return await new Promise((resolve) => {



              const response = await fetch(backendUrl, {



                    const parseResult = parseWasmError(errData.error);

                      title: 'WASM Validation Failed',
                      suggestedAction: parseResult.suggestion,
                  } else {
                    const errorMsg = errData.message || `Backend error: ${response.status}`;
                    setErrorMessage(errorMsg);
                      title: 'Analysis Failed',
                      message: errorMsg,
                      suggestedAction: 'Please check your contract code and try again.',
                  const textErr = await response.text();
                  setErrorMessage(textErr || `Server returned ${response.status}`);
                    title: 'Server Error',
                    message: textErr || `HTTP ${response.status}`,
                    suggestedAction: 'The server encountered an error. Please try again later.',
                }

                setUploadState('error');
                setDroppedFile(null);
                resolve(false);
                return;
            await response.json();
            setUploadState('success');
            if (typeof window !== 'undefined' && (window as any).triggerConfetti) {
              (window as any).triggerConfetti();
            }
            onFileReady?.(file);
            resolve(true);
          } catch (error) {
            const errorMsg = error instanceof Error ? error.message : 'Analysis request failed';
            setErrorMessage(errorMsg);
            setErrorDetails({
              title: 'Connection Error',
              message: errorMsg,
              suggestedAction: 'Please verify the backend service is running and accessible.'
            });
        };

              await response.json();
              setUploadState('success');
              onFileReady?.(file);
              resolve(true);
            } catch (error) {
              const errorMsg = error instanceof Error ? error.message : 'Analysis request failed';
                title: 'Connection Error',
                suggestedAction: 'Please verify the backend service is running and accessible.',
          };

          reader.onerror = () => {
            const errorMsg = reader.error?.message ?? 'Unable to read the selected file';
              title: 'File Read Error',
              suggestedAction: 'Please try selecting the file again.',

          reader.readAsArrayBuffer(file);
          const errorMsg = error instanceof Error ? error.message : 'Unable to start reading file';
            suggestedAction: 'Please try selecting a different file.',
      const errorMsg = error instanceof Error ? error.message : 'An unexpected error occurred';
        title: 'Submission Error',
        suggestedAction: 'Please try again.',
            const errorMessage = errData.error.message || `Backend error: ${response.status}`;
            const parseResult = parseWasmError(response, errorMessage);

              suggestedAction: 'Please check your contract code and try again.'
            suggestedAction: 'The server encountered an error. Please try again later.'
        return false;

      return true;
        suggestedAction: 'Please verify the backend service is running and accessible.'
      return false;
  }, [analyzeUrl, onFileReady]);
    },
    [backendUrl, onFileReady]
  );

  // ── Drop handling ────────────────────────────────────────────────────────────

  const onDropAccepted = useCallback(
    async (files: File[]) => {
      const file = files[0];
      setDroppedFile({ name: file.name, sizeBytes: file.size });
      setErrorMessage('');
      setErrorDetails(null);

      // Decoding happens in a Web Worker so a large contract never blocks paint
      // or input handling on the main thread.
      void (async () => {
        try {
          const arrayBuffer = await file.arrayBuffer();
          const report = await validate(arrayBuffer);

          if (!report.valid) {
            throw new Error(report.errors[0] ?? 'Failed to parse WASM metadata');
          }
      const reader = new FileReader();
      reader.onload = (event) => {
        setTimeout(async () => {
            const arrayBuffer = event.target?.result as ArrayBuffer;
            if (!arrayBuffer) throw new Error('Failed to read file content');

            if (!hasWasmMagic(arrayBuffer)) {
              throw new Error('Invalid WASM magic number. File is not a valid WebAssembly module');

            const view = new DataView(arrayBuffer);
            const version = view.getUint32(4, true);
            if (version !== 1) {
              throw new Error(`Unsupported WASM version: ${version}. Expected version 1`);

            if (enableBackendValidation) {
              await submitToBackend(file);
            } else {
              setUploadState('success');
              onFileReady?.(file);
          } catch (error) {
            const errorMsg = error instanceof Error ? error.message : 'Failed to parse WASM metadata';
            setErrorMessage(errorMsg);
            setErrorDetails({
              title: 'Invalid WASM File',
              message: errorMsg,
              suggestedAction: "Please ensure you're uploading a valid compiled Soroban contract.",
            });
            setUploadState('error');
            setDroppedFile(null);
        }, 800);
      };

      reader.onerror = () => {
        const errorMsg = reader.error?.message ?? 'Unable to read the selected file';
      if (!isWithinMaxFileSize(file.size)) {
        const errorMsg = `File is too large (${formatBytes(file.size)}). Maximum allowed size is ${formatBytes(MAX_WASM_FILE_SIZE_BYTES)}.`;
          title: 'File Too Large',
          suggestedAction: 'Please upload a smaller compiled .wasm file, or optimize your contract to reduce its size.',
        return;

      setUploadState('scanning');
      setUploadProgress(0);

        const arrayBuffer = await readFileInChunks(file, (bytesRead, totalBytes) => {
          setUploadProgress(totalBytes > 0 ? Math.round((bytesRead / totalBytes) * 100) : 100);

          validateWasmBuffer(arrayBuffer);
            suggestedAction: 'Please ensure you\'re uploading a valid compiled Soroban contract.',

          await submitToBackend(file, arrayBuffer);
        const errorMsg = error instanceof Error ? error.message : 'Unable to read the selected file';
          title: 'File Read Error',
          suggestedAction: 'Please try selecting the file again.',

          if (enableBackendValidation) {
            await submitToBackend(file);
          } else {
            setUploadState('success');
            onFileReady?.(file);
          }
        } catch (error) {
          const errorMsg =
            error instanceof Error ? error.message : 'Failed to parse WASM metadata';
          setErrorMessage(errorMsg);
          setErrorDetails({
            title: 'Invalid WASM File',
            message: errorMsg,
            suggestedAction: 'Please ensure you\'re uploading a valid compiled Soroban contract.',
          });
          setUploadState('error');
          setDroppedFile(null);
      })();
    },
    [onFileReady, enableBackendValidation, submitToBackend, validate]
  );

  const onDropRejected = useCallback((rejections: FileRejection[]) => {
    const first = rejections[0];
    const fileName = first?.file?.name ?? 'file';
    const isTooLarge = first?.errors?.some((e) => e.code === 'file-too-large');
    const ext = fileName.includes('.') ? `.${fileName.split('.').pop()}` : 'unknown type';

    const errorMsg = isTooLarge
      ? `"${fileName}" exceeds the ${MAX_WASM_SIZE / (1024 * 1024)} MB size limit`
      : `"${fileName}" was rejected — only .wasm files are accepted (got ${ext})`;

    setErrorMessage(errorMsg);
    setErrorDetails({
      title: isTooLarge ? 'File Too Large' : 'Invalid File Type',
      message: errorMsg,
      suggestedAction: 'Please upload a compiled .wasm file within size limits.',
    });
    setUploadState('error');
    setDroppedFile(null);
  }, []);

  const wasmValidator = useCallback((file: File) => {
    const extension = file.name.split('.').pop()?.toLowerCase();
    if (extension !== 'wasm') {
      return {
        code: 'file-invalid-type',
        message: `"${file.name}" was rejected — only .wasm files are accepted (got .${extension || 'unknown'})`,
      };
    }
    return null;
  }, []);

  const onDragEnter = useCallback(() => {
    if (uploadState !== 'scanning' && uploadState !== 'submitting') {
      setUploadState('hover');
    }
  }, [uploadState]);

  const onDragLeave = useCallback(() => {
    if (uploadState === 'hover') {
      setUploadState('idle');
    }
  }, [uploadState]);

  // ── Dropzone config ──────────────────────────────────────────────────────────

  const isBusy = uploadState === 'scanning' || uploadState === 'submitting';

  const { getRootProps, getInputProps, isDragActive, open } = useDropzone({
    onDropAccepted,
    onDropRejected,
    validator: wasmValidator,
    accept: { 'application/wasm': ['.wasm'] },
    maxFiles: 1,
    maxSize: MAX_WASM_SIZE,
    onDragEnter,
    onDragLeave,
    maxFiles: 1,
    noClick: uploadState === 'scanning' || uploadState === 'submitting',
    noDrag: uploadState === 'scanning' || uploadState === 'submitting',
    noClick: isBusy,
    noDrag: isBusy,
  });

  const handleReset = (e: React.MouseEvent) => {
    e.stopPropagation();
    setUploadState('idle');
    setDroppedFile(null);
    setErrorMessage('');
    setErrorDetails(null);
    setUnexpectedError(null);
    onReset?.();
  };

  const isHovered = isDragActive && uploadState !== 'scanning' && uploadState !== 'submitting';
  // ── Dynamic border & bg classes ──────────────────────────────────────────────

  const isHovered = isDragActive && !isBusy;
  const displayState = isHovered ? 'hover' : uploadState;

  const borderColor = {
    idle: 'border-slate-600 hover:border-slate-400',
    hover: 'border-sky-400 shadow-[0_0_24px_rgba(56,189,248,0.2)]',
    scanning: 'border-violet-500 shadow-[0_0_24px_rgba(167,139,250,0.25)]',
    submitting: 'border-sky-500 shadow-[0_0_24px_rgba(56,189,248,0.25)]',
    submitting: 'border-violet-500 shadow-[0_0_24px_rgba(167,139,250,0.25)]',
    success: 'border-emerald-500 shadow-[0_0_24px_rgba(52,211,153,0.2)]',
    error: 'border-red-500 shadow-[0_0_24px_rgba(248,113,113,0.2)]',
  }[displayState];

  const bgColor = {
    idle: 'bg-slate-900/60 hover:bg-slate-800/60',
    hover: 'bg-sky-950/50',
    scanning: 'bg-violet-950/40',
    submitting: 'bg-sky-950/40',
    submitting: 'bg-violet-950/40',
    success: 'bg-emerald-950/40',
    error: 'bg-red-950/30',
  }[displayState];

  return (
    <div className="w-full font-sans">
      <div
        id="wasm-upload-zone"
        {...getRootProps()}
        className={[
          'relative flex flex-col items-center justify-center',
          'border-2 border-dashed rounded-2xl p-10',
          'cursor-pointer transition-all duration-300 ease-in-out select-none',
          'min-h-[260px]',
          borderColor,
          bgColor,
        ].join(' ')}
        role="button"
        aria-label="WASM file upload zone"
      >
        <input {...getInputProps()} id="wasm-file-input" aria-label="Upload .wasm file" />

        {(displayState === 'hover' || displayState === 'scanning') && (
        {/* Animated glow ring on hover */}
        {(displayState === 'hover' || isBusy) && (
          <span
            className="absolute inset-0 rounded-2xl pointer-events-none"
            style={{
              boxShadow:
                displayState === 'hover'
                  ? '0 0 0 1px rgba(56,189,248,0.3)'
                  : '0 0 0 1px rgba(167,139,250,0.35)',
              animation: 'pulse-ring 2s ease-in-out infinite',
            }}
          />
        )}

        {(uploadState === 'idle' || uploadState === 'hover') && (
          <div className="flex flex-col items-center text-center gap-4 transition-all duration-300">
            <WasmIcon state={uploadState} />
            <div>
              <p
                className={`text-base font-semibold transition-colors duration-300 ${
                  uploadState === 'hover' ? 'text-sky-300' : 'text-slate-300'
                }`}
              >
                {uploadState === 'hover'
        {/* ── IDLE / HOVER STATE ── */}
        {(displayState === 'idle' || displayState === 'hover') && (
            <WasmIcon state={displayState} />
                  displayState === 'hover' ? 'text-sky-300' : 'text-slate-300'
                {displayState === 'hover'
                  ? 'Release to upload your .wasm file'
                  : 'Drag & drop your compiled .wasm file'}
              </p>
              <p className="text-sm text-slate-500 mt-1">
                or{' '}
                <button
                  type="button"
                  className="text-sky-400 underline underline-offset-2 hover:text-sky-300 transition-colors"
                  onClick={(e) => { e.stopPropagation(); open(); }}
                  onClick={(e) => {
                    e.stopPropagation();
                    open();
                  }}
                >
                  click to browse
                </button>
              </p>
            </div>
            <div className="flex items-center gap-2 mt-1 px-4 py-1.5 rounded-full bg-slate-800/70 border border-slate-700">
              <span className="w-2 h-2 rounded-full bg-sky-400" />
              <span className="text-xs text-slate-400 font-mono">Only .wasm files accepted</span>
            </div>
          </div>
        )}

        {uploadState === 'scanning' && (
          <div className="flex flex-col items-center text-center gap-3 w-full px-4">
            <WasmIcon state="scanning" />
            <p className="text-violet-300 font-semibold text-base tracking-wide">
              Scanning contract…
        {/* ── SCANNING / SUBMITTING STATE ── */}
        {isBusy && (
            <WasmIcon state={uploadState} />
              {uploadState === 'submitting' ? 'Analyzing contract...' : 'Scanning contract…'}
            </p>
            {droppedFile && (
              <div className="flex items-center gap-2 text-xs text-slate-400 font-mono bg-slate-800/70 px-3 py-1.5 rounded-full border border-slate-700">
                <span className="text-violet-400">📄</span>
                <span className="truncate max-w-[240px]">{droppedFile.name}</span>
                <span className="text-slate-500">·</span>
                <span>{formatBytes(droppedFile.sizeBytes)}</span>
              </div>
            )}
            <UploadProgressBar progress={uploadProgress} />
            <SpinnerDots />
            <p className="text-xs text-slate-500">Reading file in chunks · parsing WASM binary…</p>
          </div>
        )}

        {uploadState === 'submitting' && (
          <div className="flex flex-col items-center text-center gap-3 w-full px-4">
            <WasmIcon state="scanning" />
            <p className="text-sky-300 font-semibold text-base tracking-wide">
              Validating with server…
            </p>
            {droppedFile && (
              <div className="flex items-center gap-2 text-xs text-slate-400 font-mono bg-slate-800/70 px-3 py-1.5 rounded-full border border-slate-700">
                <span className="text-sky-400">📄</span>
                <span className="truncate max-w-[240px]">{droppedFile.name}</span>
                <span className="text-slate-500">·</span>
                <span>{formatBytes(droppedFile.sizeBytes)}</span>
              </div>
            )}
            <ScanningAnimation />
            <SpinnerDots />
            <p className="text-xs text-slate-500">Reading file and sending to backend…</p>
            <p className="text-xs text-slate-500">Sending to backend for analysis…</p>
          </div>
        )}

        {/* ── SUCCESS STATE ── */}
        {uploadState === 'success' && droppedFile && (
          <div className="flex flex-col items-center text-center gap-4">
            <WasmIcon state="success" />
            <div>
              <p className="text-emerald-400 font-semibold text-base">
                <SuccessIcon />
                Contract uploaded successfully
              </p>
              <p className="text-xs text-slate-500 mt-1">
                Ready for resource analysis
              </p>
            </div>

              <p className="text-xs text-slate-500 mt-1">Ready for resource analysis</p>

            {/* File info card */}
            <div className="flex items-center gap-3 bg-slate-800/80 border border-emerald-700/40 rounded-xl px-5 py-3">
              <div className="w-9 h-9 rounded-lg bg-emerald-900/50 border border-emerald-700 flex items-center justify-center flex-shrink-0">
                <span className="text-emerald-400 text-xs font-bold font-mono">WA</span>
              </div>
              <div className="text-left">
                <p className="text-sm font-medium text-slate-200 truncate max-w-[220px]">
                  {droppedFile.name}
                </p>
                <p className="text-xs text-slate-500 font-mono">{formatBytes(droppedFile.sizeBytes)}</p>
              </div>
            </div>

            <button
              type="button"
              id="wasm-upload-reset-btn"
              onClick={handleReset}
              className="text-xs text-slate-500 hover:text-slate-300 underline underline-offset-2 transition-colors mt-1"
            >
              Upload a different file
            </button>
          </div>
        )}

        {/* ── ERROR STATE ── */}
        {uploadState === 'error' && (
          <div className="flex flex-col items-center text-center gap-4">
            <WasmIcon state="error" />
            <div>
              <p className="text-red-400 font-semibold text-base">
                <ErrorIcon />
                File rejected
              </p>
              <p className="text-xs text-red-300/70 mt-1 max-w-[280px] leading-relaxed">
                {errorMessage}
              </p>
            </div>

            <div className="flex items-center gap-2 px-4 py-1.5 rounded-full bg-red-950/40 border border-red-800/50">
              <span className="w-2 h-2 rounded-full bg-red-400" />
              <span className="text-xs text-red-400 font-mono">Only .wasm files are accepted</span>
            </div>

            <button
              type="button"
              id="wasm-upload-try-again-btn"
              onClick={handleReset}
              className="mt-1 px-5 py-2 rounded-lg bg-slate-800 border border-slate-700 text-sm text-slate-300 hover:bg-slate-700 hover:text-white transition-all"
            >
              Try again
            </button>
          </div>
        )}
      </div>

      {/* Caption hint */}
      <p className="text-xs text-slate-600 text-center mt-3 font-mono">
        WASM Resource Analyzer · Soroscope · compiled Soroban contracts only
      </p>

      <style>{`
        @keyframes pulse-ring {
          0%, 100% { opacity: 0.4; }
          50%       { opacity: 1; }
        }
      `}</style>
    </div>
  );
}
