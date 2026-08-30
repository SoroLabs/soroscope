'use client';

import type { AnalyzeResponse } from './sorobantypes';
import { loadSettings, resolveEndpoint } from './userSettings';
import React, { useCallback, useState } from 'react';
import { useDropzone, FileRejection } from 'react-dropzone';
import { parseWasmError } from '../lib/errorHandling';
import { arrayBufferToBase64 } from '../lib/utils';

// ─── Constants ────────────────────────────────────────────────────────────────

const MAX_WASM_SIZE = 10 * 1024 * 1024; // 10 MB limit

/**
 * Base URL for every backend call.
 *
 * A custom indexer URL saved on the /settings page takes precedence over the
 * build-time `NEXT_PUBLIC_API_URL`, so power users can point the app at their
 * own self-hosted backend without a rebuild. Resolved per request because the
 * preference can change while the app is open.
 */
export function getApiBaseUrl(): string {
  return resolveEndpoint(loadSettings().indexerUrl, API_URL);
}

export const apiConfig = {
  baseUrl: API_URL,
  environment: process.env.NODE_ENV ?? 'development',
};

export interface ApiRequestOptions extends Omit<RequestInit, 'body'> {
  params?: Record<string, string | number | boolean | null | undefined>;
  token?: string;
  body?: BodyInit | object | null;
/** Checks for the WebAssembly magic header (\0asm) */
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

export function apiUrl(path: string, params?: ApiRequestOptions['params']): string {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  const url = new URL(`${getApiBaseUrl()}${normalizedPath}`);

  if (params) {
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        url.searchParams.set(key, String(value));
      }
    });

  return url.toString();
interface ErrorDetails {
  title: string;
  message: string;
  details?: string;
  suggestedAction?: string;
}

export interface UploadZoneProps {
  /** Called with the validated File once scanning completes */
  onFileReady?: (file: File) => void;
  /** Backend endpoint for WASM analysis */
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

// ─── Sub-components ───────────────────────────────────────────────────────────

function WasmIcon({ state }: { state: UploadState }) {
  const isActive = state === 'hover' || state === 'scanning' || state === 'submitting' || state === 'success';
  
  const strokeColor = {
    error: '#f87171',
    success: '#34d399',
    scanning: '#a78bfa',
    submitting: '#a78bfa',
    hover: '#38bdf8',
    idle: '#334155',
  }[state];

  const fillColor = {
    error: 'rgba(248,113,113,0.08)',
    success: 'rgba(52,211,153,0.08)',
    scanning: 'rgba(167,139,250,0.08)',
    submitting: 'rgba(167,139,250,0.08)',
    hover: 'rgba(56,189,248,0.08)',
    idle: 'rgba(30,41,59,0.5)',
  }[state];

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
        stroke={strokeColor}
        strokeWidth="2"
        fill={fillColor}
        className="transition-all duration-500"
      />
      <text
        x="32"
        y="35"
        textAnchor="middle"
        fontSize="11"
        fontWeight="700"
        fontFamily="monospace"
        fill={strokeColor}
        className="transition-all duration-500"
      >
        .wasm
      </text>
    </svg>
  );
}

function ScanningAnimation() {
  return (
    <div className="w-full mt-3 overflow-hidden rounded-full h-1 bg-slate-800">
      <div className="h-full rounded-full bg-gradient-to-r from-violet-500 via-fuchsia-400 to-violet-500 animate-[scan-sweep_1.6s_ease-in-out_infinite]" />
    </div>
  );
}

function SpinnerDots() {
  return (
    <div className="flex gap-1.5 items-center justify-center mt-2">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="w-1.5 h-1.5 rounded-full bg-violet-400 animate-[dot-pulse_1.2s_ease-in-out_infinite]"
          style={{ animationDelay: `${i * 0.2}s` }}
        />
      ))}
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

// ─── Main Component ───────────────────────────────────────────────────────────

export function UploadZone({
  onFileReady,
  onReset,
  backendUrl = 'http://localhost:8080/analyze/wasm',
  enableBackendValidation = true,
}: UploadZoneProps) {
  const [uploadState, setUploadState] = useState<UploadState>('idle');
  const [droppedFile, setDroppedFile] = useState<DroppedFile | null>(null);
  const [errorDetails, setErrorDetails] = useState<ErrorDetails | null>(null);

  // ── Backend submission ───────────────────────────────────────────────────────

  const submitToBackend = useCallback(
    async (file: File): Promise<boolean> => {
      try {
        setUploadState('submitting');
        const arrayBuffer = await file.arrayBuffer();
        const base64Data = arrayBufferToBase64(arrayBuffer);

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

          if (contentType?.includes('application/json')) {
            const errData = await response.json();

            if (errData.error && typeof errData.error === 'object') {
              const parseResult = parseWasmError(errData.error);
              setErrorDetails({
                title: 'WASM Validation Failed',
                message: parseResult.message,
                details: parseResult.details,
                suggestedAction: parseResult.suggestion,
              });
            } else {
              const errorMsg = errData.message || `Backend error: ${response.status}`;
              setErrorDetails({
                title: 'Analysis Failed',
                message: errorMsg,
                suggestedAction: 'Please check your contract code and try again.',
              });
            }
          } else {
            const textErr = await response.text();
            setErrorDetails({
              title: 'Server Error',
              message: textErr || `HTTP ${response.status}`,
              suggestedAction: 'The server encountered an error. Please try again later.',
            });
          }

          setUploadState('error');
          setDroppedFile(null);
          return false;
        }

        await response.json();
        setUploadState('success');
        onFileReady?.(file);
        return true;
      } catch (error) {
        const errorMsg = error instanceof Error ? error.message : 'Analysis request failed';
        setErrorDetails({
          title: 'Connection Error',
          message: errorMsg,
          suggestedAction: 'Please verify the backend service is running and accessible.',
        });
        setUploadState('error');
        setDroppedFile(null);
        return false;
      }
    },
    [backendUrl, onFileReady]
  );

  // ── Drop handling ────────────────────────────────────────────────────────────

  const onDropAccepted = useCallback(
    async (files: File[]) => {
      const file = files[0];
      if (!file) return;

      setDroppedFile({ name: file.name, sizeBytes: file.size });
      setUploadState('scanning');
      setErrorDetails(null);

      try {
        const arrayBuffer = await file.arrayBuffer();

        if (!hasWasmMagic(arrayBuffer)) {
          throw new Error('Invalid WASM magic number. File is not a valid WebAssembly module');
        }

        const view = new DataView(arrayBuffer);
        const version = view.getUint32(4, true);
        if (version !== 1) {
          throw new Error(`Unsupported WASM version: ${version}. Expected version 1`);
        }

        if (enableBackendValidation) {
          await submitToBackend(file);
        } else {
          setUploadState('success');
          onFileReady?.(file);
        }
      } catch (error) {
        const errorMsg = error instanceof Error ? error.message : 'Failed to parse WASM metadata';
        setErrorDetails({
          title: 'Invalid WASM File',
          message: errorMsg,
          suggestedAction: "Please ensure you're uploading a valid compiled Soroban contract.",
        });
        setUploadState('error');
        setDroppedFile(null);
      }
    },
    [onFileReady, enableBackendValidation, submitToBackend]
  );

  const onDropRejected = useCallback((rejections: FileRejection[]) => {
    const first = rejections[0];
    const fileName = first?.file?.name ?? 'file';
    const isTooLarge = first?.errors?.some((e) => e.code === 'file-too-large');
    const ext = fileName.includes('.') ? `.${fileName.split('.').pop()}` : 'unknown type';

    const errorMsg = isTooLarge
      ? `"${fileName}" exceeds the ${MAX_WASM_SIZE / (1024 * 1024)} MB size limit`
      : `"${fileName}" was rejected — only .wasm files are accepted (got ${ext})`;

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
    noClick: isBusy,
    noDrag: isBusy,
  });

  // ── Reset ────────────────────────────────────────────────────────────────────

  const handleReset = (e: React.MouseEvent) => {
    e.stopPropagation();
    setUploadState('idle');
    setDroppedFile(null);
    setErrorDetails(null);
    onReset?.();
  };

  // ── Dynamic border & bg classes ──────────────────────────────────────────────

  const isHovered = isDragActive && !isBusy;
  const displayState = isHovered ? 'hover' : uploadState;

  const borderColor = {
    idle: 'border-slate-600 hover:border-slate-400',
    hover: 'border-sky-400 shadow-[0_0_24px_rgba(56,189,248,0.2)]',
    scanning: 'border-violet-500 shadow-[0_0_24px_rgba(167,139,250,0.25)]',
    submitting: 'border-violet-500 shadow-[0_0_24px_rgba(167,139,250,0.25)]',
    success: 'border-emerald-500 shadow-[0_0_24px_rgba(52,211,153,0.2)]',
    error: 'border-red-500 shadow-[0_0_24px_rgba(248,113,113,0.2)]',
  }[displayState];

  const bgColor = {
    idle: 'bg-slate-900/60 hover:bg-slate-800/60',
    hover: 'bg-sky-950/50',
    scanning: 'bg-violet-950/40',
    submitting: 'bg-violet-950/40',
    success: 'bg-emerald-950/40',
    error: 'bg-red-950/30',
  }[displayState];

  // ── Render ───────────────────────────────────────────────────────────────────

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
      >
        <input {...getInputProps()} id="wasm-file-input" aria-label="Upload .wasm file" />

        {/* Glow Ring */}
        {(displayState === 'hover' || isBusy) && (
          <span className="absolute inset-0 rounded-2xl pointer-events-none animate-[pulse-ring_2s_ease-in-out_infinite] border border-sky-400/30" />
        )}

        {/* IDLE / HOVER STATE */}
        {(displayState === 'idle' || displayState === 'hover') && (
          <div className="flex flex-col items-center text-center gap-4 transition-all duration-300">
            <WasmIcon state={displayState} />
            <div>
              <p
                className={`text-base font-semibold transition-colors duration-300 ${
                  displayState === 'hover' ? 'text-sky-300' : 'text-slate-300'
                }`}
              >
                {displayState === 'hover'
                  ? 'Release to upload your .wasm file'
                  : 'Drag & drop your compiled .wasm file'}
              </p>
              <p className="text-sm text-slate-500 mt-1">
                or{' '}
                <button
                  type="button"
                  className="text-sky-400 underline underline-offset-2 hover:text-sky-300 transition-colors"
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

        {/* SCANNING / SUBMITTING STATE */}
        {isBusy && (
          <div className="flex flex-col items-center text-center gap-3 w-full px-4">
            <WasmIcon state={uploadState} />
            <p className="text-violet-300 font-semibold text-base tracking-wide">
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
            <ScanningAnimation />
            <SpinnerDots />
            <p className="text-xs text-slate-500">Parsing WASM binary · analyzing resource usage…</p>
          </div>
        )}

import { simulationQueueManager } from './requestQueue';

export const analyzeService = {
  analyze(req: AnalyzeRequest, token?: string): Promise<AnalyzeResponse> {
    return simulationQueueManager.enqueue(() =>
      apiClient.post<AnalyzeResponse>('/analyze', req, { token }),
    );
  },

  analyzeWasm(req: AnalyzeWasmRequest, token?: string): Promise<AnalyzeResponse> {
      apiClient.post<AnalyzeResponse>('/analyze/wasm', req, { token }),
};

        {/* SUCCESS STATE */}
        {uploadState === 'success' && droppedFile && (
          <div className="flex flex-col items-center text-center gap-4">
            <WasmIcon state="success" />
            <div>
              <p className="text-emerald-400 font-semibold text-base">
                <SuccessIcon />
                Contract uploaded successfully
              </p>
              <p className="text-xs text-slate-500 mt-1">Ready for resource analysis</p>
            </div>

            <div className="flex items-center gap-3 bg-slate-800/80 border border-emerald-700/40 rounded-xl px-5 py-3">
              <div className="w-9 h-9 rounded-lg bg-emerald-900/50 border border-emerald-700 flex items-center justify-center flex-shrink-0">
                <span className="text-emerald-400 text-xs font-bold font-mono">WA</span>
              <div className="text-left">
                <p className="text-sm font-medium text-slate-200 truncate max-w-[220px]">
                  {droppedFile.name}
                <p className="text-xs text-slate-500 font-mono">{formatBytes(droppedFile.sizeBytes)}</p>

            <button
              type="button"
              id="wasm-upload-reset-btn"
              onClick={handleReset}
              className="text-xs text-slate-500 hover:text-slate-300 underline underline-offset-2 transition-colors mt-1"
            >
              Upload a different file
            </button>
        )}

        {/* ERROR STATE */}
        {uploadState === 'error' && errorDetails && (
          <div className="flex flex-col items-center text-center gap-3 max-w-md">
            <WasmIcon state="error" />
              <p className="text-red-400 font-semibold text-base">
                <ErrorIcon />
                {errorDetails.title}
              <p className="text-xs text-red-300/80 mt-1 leading-relaxed">
                {errorDetails.message}
              {errorDetails.details && (
                <p className="text-xs text-slate-400 font-mono mt-1 bg-slate-950/60 p-2 rounded border border-slate-800/80 text-left overflow-x-auto max-w-full">
                  {errorDetails.details}
              {errorDetails.suggestedAction && (
                <p className="text-xs text-amber-300/70 mt-2 italic">
                  💡 {errorDetails.suggestedAction}

              id="wasm-upload-try-again-btn"
              className="mt-2 px-5 py-2 rounded-lg bg-slate-800 border border-slate-700 text-sm text-slate-300 hover:bg-slate-700 hover:text-white transition-all"
              Try again

      <p className="text-xs text-slate-600 text-center mt-3 font-mono">
        WASM Resource Analyzer · Soroscope · compiled Soroban contracts only

      {/* Global Style Animations */}
      <style jsx global>{`
        @keyframes scan-sweep {
          0% { transform: translateX(-100%); }
          100% { transform: translateX(200%); }
        }
        @keyframes dot-pulse {
          0%, 80%, 100% { opacity: 0.2; transform: scale(0.8); }
          40% { opacity: 1; transform: scale(1.2); }
        @keyframes pulse-ring {
          0%, 100% { opacity: 0.4; }
          50% { opacity: 1; }
      `}</style>
