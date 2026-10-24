'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  AUTH_FAILURE_CLOSE_CODE,
  RECONNECTABLE_CLOSE_CODES,
  isAuthFailure,
  shouldReconnect,
} = require('../websocketClose');

test('only abnormal close (1006) and going away (1001) are retried', () => {
  assert.deepEqual([...RECONNECTABLE_CLOSE_CODES].sort(), [1001, 1006]);
  assert.equal(shouldReconnect(1006), true);
  assert.equal(shouldReconnect(1001), true);
});

test('auth failure (4003) is never retried', () => {
  assert.equal(AUTH_FAILURE_CLOSE_CODE, 4003);
  assert.equal(isAuthFailure(4003), true);
  assert.equal(shouldReconnect(4003), false);
});

test('normal, policy and other application close codes are not retried', () => {
  for (const code of [1000, 1002, 1003, 1008, 1011, 4000, 4001, 4004]) {
    assert.equal(shouldReconnect(code), false, `code ${code}`);
    assert.equal(isAuthFailure(code), false, `code ${code}`);
  }
});
