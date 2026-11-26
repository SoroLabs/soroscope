'use client';

import { useCallback, useEffect, useRef } from 'react';

import { validateWasmModule } from '../lib/wasmValidation';
import type { WasmValidationReport } from '../lib/wasmValidation';

interface PendingJob {
  resolve: (report: WasmValidationReport) => void;
  reject: (error: Error) => void;
}

/** How long to wait for the worker's boot handshake before giving up on it. */
const PROBE_TIMEOUT_MS = 2000;

/** Served as a static asset from `public/`, so no bundler config is required. */
const WORKER_URL = '/wasm/wasmWorker.js';

export interface UseWasmValidationWorker {
  /**
   * Validate a WASM buffer, off the main thread when possible.
   *
   * Falls back to synchronous main-thread validation (the exact same function)
   * when the environment has no Worker support or the worker fails to boot, so
   * the result is identical either way.
   */
  validate: (buffer: ArrayBuffer, maxBytes?: number) => Promise<WasmValidationReport>;
  /** True once a worker has completed its handshake and is handling jobs. */
  isOffloaded: () => boolean;
}

function supportsWorker(): boolean {
  return typeof window !== 'undefined' && typeof window.Worker !== 'undefined';
}

/**
 * Owns a single long-lived Web Worker for WASM parsing.
 *
 * The worker is created lazily on first use — pages that never upload a
 * contract do not pay for it — and is only trusted with real work after it
 * answers a ping. That handshake keeps a blocked worker (CSP, unsupported
 * browser) from failing the upload: we quietly use the main thread instead.
 */
export function useWasmValidationWorker(): UseWasmValidationWorker {
  const workerRef = useRef<Worker | null>(null);
  const readyRef = useRef<Promise<Worker | null> | null>(null);
  const pendingRef = useRef<Map<string, PendingJob>>(new Map());
  const jobCounterRef = useRef(0);

  const ensureWorker = useCallback((): Promise<Worker | null> => {
    if (readyRef.current) return readyRef.current;

    readyRef.current = new Promise<Worker | null>((resolve) => {
      if (!supportsWorker()) {
        resolve(null);
        return;
      }

      let worker: Worker;
      try {
        worker = new Worker(WORKER_URL);
      } catch {
        // CSP or a sandboxed context refused the worker.
        resolve(null);
        return;
      }

      let settled = false;

      const giveUp = () => {
        if (settled) return;
        settled = true;
        worker.terminate();
        resolve(null);
      };

      const timer = window.setTimeout(giveUp, PROBE_TIMEOUT_MS);

      const handleProbe = (event: MessageEvent) => {
        if (settled || event.data?.type !== 'pong') return;
        settled = true;
        window.clearTimeout(timer);
        worker.removeEventListener('message', handleProbe);
        worker.removeEventListener('error', giveUp);

        worker.addEventListener('message', (message: MessageEvent) => {
          const data = message.data as
            | { id: string; type: 'result'; report: WasmValidationReport }
            | { id: string; type: 'error'; message: string };

          const job = pendingRef.current.get(data.id);
          if (!job) return;
          pendingRef.current.delete(data.id);

          if (data.type === 'result') {
            job.resolve(data.report);
          } else {
            job.reject(new Error(data.message));
          }
        });

        worker.addEventListener('error', (event: ErrorEvent) => {
          const error = new Error(event.message || 'WASM worker crashed');
          pendingRef.current.forEach((job) => job.reject(error));
          pendingRef.current.clear();
        });

        workerRef.current = worker;
        resolve(worker);
      };

      worker.addEventListener('message', handleProbe);
      worker.addEventListener('error', giveUp);
      worker.postMessage({ type: 'ping' });
    });

    return readyRef.current;
  }, []);

  useEffect(() => {
    const pending = pendingRef.current;
    return () => {
      workerRef.current?.terminate();
      workerRef.current = null;
      pending.clear();
    };
  }, []);

  const validate = useCallback(
    async (buffer: ArrayBuffer, maxBytes?: number): Promise<WasmValidationReport> => {
      const worker = await ensureWorker();

      if (!worker) {
        return validateWasmModule(buffer, { maxBytes });
      }

      jobCounterRef.current += 1;
      const id = `wasm-job-${jobCounterRef.current}`;

      return new Promise<WasmValidationReport>((resolve, reject) => {
        pendingRef.current.set(id, { resolve, reject });
        // Transfer (not copy) the buffer so large uploads stay cheap. Safe here
        // because the worker already proved it is alive.
        worker.postMessage({ id, type: 'validate', buffer, maxBytes }, [buffer]);
      });
    },
    [ensureWorker],
  );

  const isOffloaded = useCallback(() => workerRef.current !== null, []);

  return { validate, isOffloaded };
}
