/**
 * Dedicated Web Worker for WASM decoding and validation.
 *
 * Parsing a multi-megabyte contract on the main thread blocks paint and input
 * handling, which showed up as typing lag while an upload was in flight. The
 * whole decode now happens here and only the (small) JSON report crosses back
 * over the message channel.
 *
 * This is a classic worker served straight from `public/`, deliberately not a
 * bundler worker entry: `importScripts` resolves at runtime, so the same file
 * works identically under webpack, Turbopack and `next dev`.
 *
 * Protocol
 *   in : { type: 'ping' }
 *   out: { type: 'pong' }
 *   in : { id: string, type: 'validate', buffer: ArrayBuffer, maxBytes?: number }
 *   out: { id: string, type: 'result', report: WasmValidationReport }
 *        { id: string, type: 'error',  message: string }
 *
 * The caller only sends real work after `pong` comes back, so a worker that
 * cannot boot degrades to main-thread validation instead of failing uploads.
 */

importScripts('/wasm/wasmValidation.js');

const { validateWasmModule } = self.SoroscopeWasmValidation;

self.onmessage = (event) => {
  const data = event && event.data;
  if (!data) return;

  if (data.type === 'ping') {
    self.postMessage({ type: 'pong' });
    return;
  }

  if (data.type !== 'validate') {
    return;
  }

  const { id, buffer, maxBytes } = data;

  try {
    const report = validateWasmModule(buffer, { maxBytes });
    self.postMessage({ id, type: 'result', report });
  } catch (error) {
    self.postMessage({
      id,
      type: 'error',
      message: error instanceof Error ? error.message : 'WASM validation failed',
    });
  }
};
