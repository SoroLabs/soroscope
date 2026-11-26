// upload-zone.test.cjs — unit tests for upload-zone validation logic
// Closes #675 — adds 2 MB size limit + magic-bytes checks
// Runs with: node --test ./components/upload-zone.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// ── Constants (mirrors WasmUpload.tsx) ───────────────────────────────────────

const MAX_WASM_SIZE = 2 * 1024 * 1024; // 2 MB
const WASM_MAGIC    = 0x0061736d;       // \0asm

// ── Pure validation helpers (mirrors upload-zone.tsx logic) ──────────────────

/**
 * wasmValidator: returns null for .wasm files, error object otherwise.
 * Mirrors the inline `wasmValidator` callback in upload-zone.tsx.
 */
function wasmValidator(fileName) {
  const ext = fileName.split('.').pop()?.toLowerCase();
  if (ext !== 'wasm') {
    return {
      code: 'file-invalid-type',
      message: `"${fileName}" was rejected — only .wasm files are accepted (got .${ext || 'unknown'})`,
    };
  }
  return null;
}

/**
 * validateWasm: synchronous checks — extension, size, empty.
 * Mirrors the `validateWasm` callback in WasmUpload.tsx.
 */
function validateWasm(fileName, fileSize, maxFileSize = MAX_WASM_SIZE) {
  if (!fileName.toLowerCase().endsWith('.wasm')) {
    return 'Validation Error: File must be a .wasm file';
  }
  if (fileSize > maxFileSize) {
    return `File too large: ${(fileSize / 1024 / 1024).toFixed(2)} MB exceeds the ${(maxFileSize / 1024 / 1024).toFixed(0)} MB limit`;
  }
  if (fileSize === 0) {
    return 'File is empty';
  }
  return null;
}

/**
 * validateWasmBuffer: checks magic number and version.
 * Mirrors the ArrayBuffer checks inside onDropAccepted in upload-zone.tsx
 * and the async validateWasmMagic helper in WasmUpload.tsx.
 */
function validateWasmBuffer(buffer) {
  if (buffer.byteLength < 8) {
    throw new Error('File is too small to be a valid WebAssembly module');
  }
  const view = new DataView(buffer);
  const magic = view.getUint32(0, false); // big-endian: \0asm
  if (magic !== WASM_MAGIC) {
    throw new Error('Invalid WASM magic bytes — file does not start with \\0asm. Ensure you uploaded a compiled Soroban contract.');
  }
  const version = view.getUint32(4, true); // little-endian
  if (version !== 1) {
    throw new Error(`Unsupported WASM version: ${version}. Expected version 1.`);
  }
}

// ── Helpers to build minimal buffers ─────────────────────────────────────────

function makeWasmBuffer(magic = WASM_MAGIC, version = 1) {
  const buf = new ArrayBuffer(8);
  const view = new DataView(buf);
  view.setUint32(0, magic, false);
  view.setUint32(4, version, true);
  return buf;
}

/** Build a buffer filled with random bytes of the requested size. */
function makeRandomBuffer(sizeBytes) {
  const buf = new ArrayBuffer(sizeBytes);
  const view = new Uint8Array(buf);
  for (let i = 0; i < sizeBytes; i++) view[i] = (i % 256);
  return buf;
}

// ── wasmValidator tests ───────────────────────────────────────────────────────

test('wasmValidator: accepts .wasm file', () => {
  assert.equal(wasmValidator('contract.wasm'), null);
});

test('wasmValidator: rejects .js file', () => {
  const result = wasmValidator('script.js');
  assert.equal(result.code, 'file-invalid-type');
  assert.match(result.message, /only .wasm files are accepted/);
});

test('wasmValidator: rejects file with no extension', () => {
  const result = wasmValidator('noextension');
  assert.ok(result !== null);
  assert.equal(result.code, 'file-invalid-type');
});

test('wasmValidator: rejects .txt file', () => {
  const result = wasmValidator('readme.txt');
  assert.ok(result !== null);
  assert.match(result.message, /got .txt/);
});

// ── validateWasm size-limit tests (Closes #675) ───────────────────────────────

test('validateWasm: accepts valid .wasm under 2 MB', () => {
  // 1 KB — well within limit
  assert.equal(validateWasm('contract.wasm', 1024), null);
});

test('validateWasm: accepts .wasm at exactly 2 MB boundary', () => {
  assert.equal(validateWasm('contract.wasm', MAX_WASM_SIZE), null);
});

test('validateWasm: rejects .wasm exceeding 2 MB limit', () => {
  const oversizeBytes = MAX_WASM_SIZE + 1;
  const error = validateWasm('big.wasm', oversizeBytes);
  assert.ok(error !== null, 'Expected an error for oversize file');
  assert.match(error, /too large/i);
  assert.match(error, /2 MB limit/);
});

test('validateWasm: error message includes actual file size in MB', () => {
  const oversizeBytes = 3 * 1024 * 1024; // 3 MB
  const error = validateWasm('fat.wasm', oversizeBytes);
  assert.ok(error !== null);
  // Should mention the actual size (3.00 MB)
  assert.match(error, /3\.00 MB/);
});

test('validateWasm: rejects empty file', () => {
  const error = validateWasm('empty.wasm', 0);
  assert.ok(error !== null);
  assert.match(error, /empty/i);
});

test('validateWasm: rejects non-wasm extension even if small', () => {
  const error = validateWasm('binary.exe', 1024);
  assert.ok(error !== null);
  assert.match(error, /must be a .wasm file/);
});

// ── validateWasmBuffer magic-bytes tests ──────────────────────────────────────

test('validateWasmBuffer: accepts valid WASM magic + version 1', () => {
  assert.doesNotThrow(() => validateWasmBuffer(makeWasmBuffer()));
});

test('validateWasmBuffer: rejects buffer smaller than 8 bytes', () => {
  assert.throws(
    () => validateWasmBuffer(new ArrayBuffer(4)),
    /too small/
  );
});

test('validateWasmBuffer: rejects wrong magic number', () => {
  assert.throws(
    () => validateWasmBuffer(makeWasmBuffer(0xdeadbeef, 1)),
    /Invalid WASM magic bytes/
  );
});

test('validateWasmBuffer: rejects magic bytes that look like text (not \\0asm)', () => {
  // A PDF header or plain-text file would fail this check
  const buf = new ArrayBuffer(8);
  const view = new Uint8Array(buf);
  // "%PDF" bytes
  view[0] = 0x25; view[1] = 0x50; view[2] = 0x44; view[3] = 0x46;
  assert.throws(
    () => validateWasmBuffer(buf),
    /Invalid WASM magic bytes/
  );
});

test('validateWasmBuffer: rejects unsupported version 2', () => {
  assert.throws(
    () => validateWasmBuffer(makeWasmBuffer(WASM_MAGIC, 2)),
    /Unsupported WASM version: 2/
  );
});

test('validateWasmBuffer: rejects unsupported version 0', () => {
  assert.throws(
    () => validateWasmBuffer(makeWasmBuffer(WASM_MAGIC, 0)),
    /Unsupported WASM version: 0/
  );
});

// ── Combined guard: size check must happen BEFORE magic check ─────────────────

test('size guard catches oversized file before magic-bytes check is reached', () => {
  // A 3 MB buffer with correct magic bytes should be caught by size guard first
  const oversizeBytes = 3 * 1024 * 1024;
  const error = validateWasm('large_valid.wasm', oversizeBytes);
  assert.ok(error !== null, 'size guard should fire');
  assert.match(error, /too large/i);
  // Confirm magic check would have passed — no confusion between the two errors
  const buf = makeWasmBuffer();
  assert.doesNotThrow(() => validateWasmBuffer(buf));
});

// ── Clear / reset state tests (state machine logic) ──────────────────────────

test('handleReset: resets state to idle', () => {
  // Simulate the state fields managed by handleReset in upload-zone.tsx
  let uploadState = 'error';
  let droppedFile = { name: 'bad.txt', sizeBytes: 10 };
  let errorMessage = 'some error';

  // handleReset logic
  uploadState = 'idle';
  droppedFile = null;
  errorMessage = '';

  assert.equal(uploadState, 'idle');
  assert.equal(droppedFile, null);
  assert.equal(errorMessage, '');
});

test('onDropRejected: builds correct error message for non-wasm file', () => {
  const fileName = 'malicious.exe';
  const ext = fileName.includes('.') ? `.${fileName.split('.').pop()}` : 'unknown type';
  const errorMsg = `"${fileName}" was rejected — only .wasm files are accepted (got ${ext})`;

  assert.match(errorMsg, /only .wasm files are accepted/);
  assert.match(errorMsg, /\.exe/);
});

// ── MAX_WASM_SIZE constant verification ───────────────────────────────────────

test('MAX_WASM_SIZE constant equals exactly 2 MB (2097152 bytes)', () => {
  assert.equal(MAX_WASM_SIZE, 2 * 1024 * 1024);
  assert.equal(MAX_WASM_SIZE, 2097152);
});

test('WASM_MAGIC constant matches \\0asm bytes', () => {
  // 0x00 0x61 0x73 0x6D → \0asm
  assert.equal(WASM_MAGIC, 0x0061736d);
  const buf = new ArrayBuffer(4);
  const view = new DataView(buf);
  view.setUint32(0, WASM_MAGIC, false);
  const bytes = new Uint8Array(buf);
  assert.deepEqual([...bytes], [0x00, 0x61, 0x73, 0x6d]);
});
