// wasmWorker.test.cjs — protocol tests for the WASM validation Web Worker
// Closes Issue #621
// Runs with: node --test ./lib/wasmWorker.test.cjs
//
// The worker is a classic script served from public/, so it cannot simply be
// required. Instead it is executed in a VM with a stubbed `self`, which
// exercises the exact file the browser loads.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const WORKER_PATH = path.join(__dirname, '..', 'public', 'wasm', 'wasmWorker.js');
const validation = require('./wasmValidation');

/** Boot the worker script against a fake `self` and return a driver. */
function bootWorker() {
  const posted = [];
  const sandbox = {
    postMessage: (message) => posted.push(message),
    SoroscopeWasmValidation: validation,
    // importScripts is a no-op here; the real logic is injected above.
    importScripts: () => {},
  };
  sandbox.self = sandbox; // workers expose the global scope as `self`

  const source = fs.readFileSync(WORKER_PATH, 'utf8');
  vm.createContext(sandbox);
  vm.runInContext(source, sandbox, { filename: 'wasmWorker.js' });

  return {
    posted,
    send: (data) => sandbox.onmessage({ data }),
  };
}

function validModule() {
  // Header + an export section declaring one function named "transfer".
  const name = Buffer.from('transfer', 'utf8');
  const payload = [0x01, name.length, ...name, 0x00, 0x00];
  return Uint8Array.from([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    0x07, payload.length, ...payload,
  ]);
}

test('worker: answers the boot handshake with pong', () => {
  const worker = bootWorker();
  worker.send({ type: 'ping' });
  assert.equal(worker.posted.length, 1);
  assert.equal(worker.posted[0].type, 'pong');
});

test('worker: returns a decode report for a valid module', () => {
  const worker = bootWorker();
  worker.send({ id: 'job-1', type: 'validate', buffer: validModule() });

  assert.equal(worker.posted.length, 1);
  const message = worker.posted[0];
  assert.equal(message.id, 'job-1');
  assert.equal(message.type, 'result');
  assert.equal(message.report.valid, true);
  assert.deepEqual(
    message.report.exports.map((e) => e.name),
    ['transfer'],
  );
});

test('worker: reports invalid bytecode as an unsuccessful result, not a crash', () => {
  const worker = bootWorker();
  worker.send({ id: 'job-2', type: 'validate', buffer: Buffer.from('not wasm at all') });

  const message = worker.posted[0];
  assert.equal(message.type, 'result');
  assert.equal(message.report.valid, false);
  assert.ok(message.report.errors.length > 0);
});

test('worker: honours the maxBytes limit passed by the caller', () => {
  const worker = bootWorker();
  worker.send({ id: 'job-3', type: 'validate', buffer: validModule(), maxBytes: 4 });

  assert.equal(worker.posted[0].report.valid, false);
  assert.match(worker.posted[0].report.errors[0], /exceeds/i);
});

test('worker: ignores unknown and empty messages', () => {
  const worker = bootWorker();
  worker.send({ type: 'something-else' });
  worker.send(null);
  assert.equal(worker.posted.length, 0);
});

test('worker: replies on the same job id it was given', () => {
  const worker = bootWorker();
  worker.send({ id: 'a', type: 'validate', buffer: validModule() });
  worker.send({ id: 'b', type: 'validate', buffer: validModule() });

  assert.deepEqual(
    worker.posted.map((m) => m.id),
    ['a', 'b'],
  );
});
