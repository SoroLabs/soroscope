// wasmValidation.test.cjs — unit tests for the Web Worker WASM decoder
// Closes Issue #621
// Runs with: node --test ./lib/wasmValidation.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  hasWasmMagic,
  readVarUint32,
  parseWasmSections,
  validateWasmModule,
  extractContractFunctions,
} = require('./wasmValidation');

// ── Fixtures ────────────────────────────────────────────────────────────────

const HEADER = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

function encodeString(text) {
  const bytes = Array.from(Buffer.from(text, 'utf8'));
  return [bytes.length, ...bytes];
}

/** Build a minimal module exporting the given function names. */
function buildModule(exportNames = [], extraSections = []) {
  const entries = exportNames.flatMap((name, index) => [
    ...encodeString(name),
    0x00, // kind: function
    index,
  ]);
  const exportPayload = [exportNames.length, ...entries];

  const sections = [
    ...extraSections,
    ...(exportNames.length > 0 ? [[0x07, exportPayload]] : []),
  ];

  const body = sections.flatMap(([id, payload]) => [id, payload.length, ...payload]);
  return Uint8Array.from([...HEADER, ...body]);
}

// ── Magic number / header ───────────────────────────────────────────────────

test('hasWasmMagic: accepts a \\0asm header', () => {
  assert.equal(hasWasmMagic(Uint8Array.from(HEADER)), true);
});

test('hasWasmMagic: rejects arbitrary bytes and short buffers', () => {
  assert.equal(hasWasmMagic(Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8])), false);
  assert.equal(hasWasmMagic(Uint8Array.from([0x00, 0x61])), false);
});

test('validateWasmModule: rejects an empty file', () => {
  const report = validateWasmModule(new Uint8Array(0));
  assert.equal(report.valid, false);
  assert.match(report.errors[0], /empty/i);
});

test('validateWasmModule: rejects a non-WASM file', () => {
  const report = validateWasmModule(Buffer.from('this is definitely not wasm'));
  assert.equal(report.valid, false);
  assert.match(report.errors[0], /magic number/i);
});

test('validateWasmModule: rejects an unsupported binary version', () => {
  const bytes = Uint8Array.from([...HEADER]);
  bytes[4] = 0x02;
  const report = validateWasmModule(bytes);
  assert.equal(report.valid, false);
  assert.equal(report.version, 2);
  assert.match(report.errors[0], /version/i);
});

test('validateWasmModule: enforces the maxBytes limit', () => {
  const report = validateWasmModule(buildModule(['transfer']), { maxBytes: 4 });
  assert.equal(report.valid, false);
  assert.match(report.errors[0], /exceeds/i);
});

// ── LEB128 ──────────────────────────────────────────────────────────────────

test('readVarUint32: decodes single and multi byte LEB128 values', () => {
  assert.deepEqual(readVarUint32(Uint8Array.from([0x00]), 0), { value: 0, next: 1 });
  assert.deepEqual(readVarUint32(Uint8Array.from([0x7f]), 0), { value: 127, next: 1 });
  assert.deepEqual(readVarUint32(Uint8Array.from([0x80, 0x01]), 0), { value: 128, next: 2 });
  assert.deepEqual(readVarUint32(Uint8Array.from([0xe5, 0x8e, 0x26]), 0), {
    value: 624485,
    next: 3,
  });
});

test('readVarUint32: throws when the buffer ends mid-integer', () => {
  assert.throws(() => readVarUint32(Uint8Array.from([0x80]), 0), /Unexpected end of buffer/);
});

// ── Section walking ─────────────────────────────────────────────────────────

test('parseWasmSections: lists sections with names and sizes', () => {
  const module = buildModule(['transfer'], [[0x01, [0x00]], [0x0a, [0x00]]]);
  const sections = parseWasmSections(module);
  assert.deepEqual(
    sections.map((s) => s.name),
    ['type', 'code', 'export'],
  );
  assert.equal(sections[0].size, 1);
});

test('parseWasmSections: throws when a section overruns the buffer', () => {
  const truncated = Uint8Array.from([...HEADER, 0x07, 0x40, 0x01]);
  assert.throws(() => parseWasmSections(truncated), /claims 64 bytes/);
});

test('validateWasmModule: reports a decode failure instead of throwing', () => {
  const truncated = Uint8Array.from([...HEADER, 0x07, 0x40, 0x01]);
  const report = validateWasmModule(truncated);
  assert.equal(report.valid, false);
  assert.equal(report.errors.length, 1);
});

// ── Export extraction ───────────────────────────────────────────────────────

test('validateWasmModule: accepts a well formed module and decodes its exports', () => {
  const report = validateWasmModule(buildModule(['transfer', 'balance']));
  assert.equal(report.valid, true);
  assert.equal(report.version, 1);
  assert.equal(report.errors.length, 0);
  assert.deepEqual(
    report.exports.map((e) => e.name),
    ['transfer', 'balance'],
  );
  assert.equal(report.exports[0].kind, 'function');
  assert.equal(report.exports[1].index, 1);
});

test('validateWasmModule: warns when a module exports nothing', () => {
  const report = validateWasmModule(buildModule([]));
  assert.equal(report.valid, true);
  assert.ok(report.warnings.some((w) => /exports nothing/i.test(w)));
});

test('validateWasmModule: accepts a plain ArrayBuffer', () => {
  const module = buildModule(['transfer']);
  const report = validateWasmModule(module.buffer.slice(0));
  assert.equal(report.valid, true);
  assert.equal(report.byteLength, module.length);
});

test('extractContractFunctions: returns sorted public function exports only', () => {
  const report = validateWasmModule(buildModule(['transfer', '_start', 'balance']));
  assert.deepEqual(extractContractFunctions(report), ['balance', 'transfer']);
});

test('extractContractFunctions: tolerates a missing report', () => {
  assert.deepEqual(extractContractFunctions(null), []);
  assert.deepEqual(extractContractFunctions({}), []);
});
