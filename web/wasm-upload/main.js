// Main-thread entry for the WASM upload UI.
// Heavy WASM bytecode decoding/validation is offloaded to a dedicated Web
// Worker (see ./wasm-decoder.worker.js) so the UI thread stays responsive
// (60fps) even for large (~10MB) binaries.

const worker = new Worker(new URL('./wasm-decoder.worker.js', import.meta.url), {
  type: 'module',
});

let nextRequestId = 0;
const pending = new Map();

worker.addEventListener('message', (event) => {
  const { id, ok, result, error } = event.data || {};
  const entry = pending.get(id);
  if (!entry) return;
  pending.delete(id);
  if (ok) {
    entry.resolve(result);
  } else {
    entry.reject(new Error(error || 'WASM decoding failed'));
  }
});

worker.addEventListener('error', (event) => {
  const err = new Error(event.message || 'WASM worker error');
  for (const { reject } of pending.values()) reject(err);
  pending.clear();
});

/**
 * Decode and validate a WASM binary off the main thread.
 * @param {ArrayBuffer} buffer raw WASM bytes
 * @returns {Promise<object>} decoded module metadata
 */
export function decodeWasm(buffer) {
  const id = nextRequestId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    // Transfer the buffer to avoid copying large payloads.
    worker.postMessage({ id, buffer }, [buffer]);
  });
}

/**
 * Wire up a file input so selecting a WASM file decodes it in the worker
 * without blocking the UI thread.
 * @param {HTMLInputElement} input
 * @param {(result: object) => void} [onDecoded]
 * @param {(error: Error) => void} [onError]
 */
export function attachWasmUpload(input, onDecoded, onError) {
  input.addEventListener('change', async () => {
    const file = input.files && input.files[0];
    if (!file) return;
    try {
      const buffer = await file.arrayBuffer();
      const result = await decodeWasm(buffer);
      if (onDecoded) onDecoded(result);
    } catch (err) {
      if (onError) onError(err);
    }
  });
}
