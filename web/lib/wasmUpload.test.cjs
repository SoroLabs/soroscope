// wasmUpload.test.cjs — unit tests for chunked file reading and WASM validation
// closes #598
// Runs with: node --test ./lib/wasmUpload.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// Node has no global FileReader; readFileInChunks only needs onload/onerror
// and readAsArrayBuffer(blob), so a minimal shim over Blob#arrayBuffer() is
// enough to exercise the real chunking implementation end-to-end.
if (typeof globalThis.FileReader === 'undefined') {
  globalThis.FileReader = class FileReaderPolyfill {
    constructor() {
      this.onload = null;
      this.onerror = null;
      this.error = null;
    }

    readAsArrayBuffer(blob) {
      blob
        .arrayBuffer()
        .then((buffer) => {
          this.onload?.({ target: { result: buffer } });
        })
        .catch((err) => {
          this.error = err;
          this.onerror?.();
        });
    }
  };
}

const {
  MAX_WASM_FILE_SIZE_BYTES,
  computeChunkRanges,
  isWithinMaxFileSize,
  validateWasmBuffer,
  readFileInChunks,
} = require('./wasmUpload');

function makeWasmBuffer(magic = 0x0061736d, version = 1) {
  const buf = new ArrayBuffer(8);
  const view = new DataView(buf);
  view.setUint32(0, magic, false);
  view.setUint32(4, version, true);
  return buf;
}

// ── computeChunkRanges ────────────────────────────────────────────────────

test('computeChunkRanges: returns no ranges for an empty file', () => {
  assert.deepEqual(computeChunkRanges(0, 1024), []);
});

test('computeChunkRanges: single range when file fits in one chunk', () => {
  assert.deepEqual(computeChunkRanges(100, 1024), [{ start: 0, end: 100 }]);
});

test('computeChunkRanges: splits into full chunks plus a shorter remainder', () => {
  // 5 bytes total, 2-byte chunks -> [0,2) [2,4) [4,5)
  assert.deepEqual(computeChunkRanges(5, 2), [
    { start: 0, end: 2 },
    { start: 2, end: 4 },
    { start: 4, end: 5 },
  ]);
});

test('computeChunkRanges: exact multiple of chunk size has no short final chunk', () => {
  assert.deepEqual(computeChunkRanges(6, 2), [
    { start: 0, end: 2 },
    { start: 2, end: 4 },
    { start: 4, end: 6 },
  ]);
});

test('computeChunkRanges: throws for non-positive chunk size', () => {
  assert.throws(() => computeChunkRanges(100, 0), /chunkSizeBytes must be greater than 0/);
  assert.throws(() => computeChunkRanges(100, -5), /chunkSizeBytes must be greater than 0/);
});

// ── isWithinMaxFileSize ────────────────────────────────────────────────────

test('isWithinMaxFileSize: accepts sizes at or under the default cap', () => {
  assert.equal(isWithinMaxFileSize(0), true);
  assert.equal(isWithinMaxFileSize(MAX_WASM_FILE_SIZE_BYTES), true);
});

test('isWithinMaxFileSize: rejects sizes over the default cap', () => {
  assert.equal(isWithinMaxFileSize(MAX_WASM_FILE_SIZE_BYTES + 1), false);
});

test('isWithinMaxFileSize: honors a custom cap', () => {
  assert.equal(isWithinMaxFileSize(2048, 1024), false);
  assert.equal(isWithinMaxFileSize(1024, 1024), true);
});

// ── validateWasmBuffer ─────────────────────────────────────────────────────

test('validateWasmBuffer: accepts valid WASM magic + version 1', () => {
  assert.doesNotThrow(() => validateWasmBuffer(makeWasmBuffer()));
});

test('validateWasmBuffer: rejects buffer smaller than 8 bytes', () => {
  assert.throws(() => validateWasmBuffer(new ArrayBuffer(4)), /too small/);
});

test('validateWasmBuffer: rejects wrong magic number', () => {
  assert.throws(
    () => validateWasmBuffer(makeWasmBuffer(0xdeadbeef, 1)),
    /Invalid WASM magic number/
  );
});

test('validateWasmBuffer: rejects unsupported version', () => {
  assert.throws(
    () => validateWasmBuffer(makeWasmBuffer(0x0061736d, 2)),
    /Unsupported WASM version: 2/
  );
});

// ── readFileInChunks ───────────────────────────────────────────────────────

test('readFileInChunks: reassembles a file read across multiple chunks', async () => {
  const bytes = Uint8Array.from({ length: 10 }, (_, i) => i);
  const file = new File([bytes], 'contract.wasm');

  const progressUpdates = [];
  const result = await readFileInChunks(
    file,
    (bytesRead, totalBytes) => progressUpdates.push([bytesRead, totalBytes]),
    3 // small chunk size to force multiple reads
  );

  assert.deepEqual(Array.from(new Uint8Array(result)), Array.from(bytes));
  assert.deepEqual(progressUpdates, [
    [3, 10],
    [6, 10],
    [9, 10],
    [10, 10],
  ]);
});

test('readFileInChunks: resolves an empty buffer for a zero-byte file', async () => {
  const file = new File([], 'empty.wasm');
  const result = await readFileInChunks(file);
  assert.equal(result.byteLength, 0);
});

test('readFileInChunks: a single chunk covers files smaller than the chunk size', async () => {
  const bytes = Uint8Array.from([1, 2, 3, 4]);
  const file = new File([bytes], 'small.wasm');

  const progressUpdates = [];
  const result = await readFileInChunks(
    file,
    (bytesRead, totalBytes) => progressUpdates.push([bytesRead, totalBytes]),
    1024
  );

  assert.deepEqual(Array.from(new Uint8Array(result)), Array.from(bytes));
  assert.deepEqual(progressUpdates, [[4, 4]]);
});
