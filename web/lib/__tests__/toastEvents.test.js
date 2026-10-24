'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const { TOAST_EVENT, createToast, emitToast } = require('../toastEvents');

test('createToast assigns unique ids and keeps known types', () => {
  const a = createToast({ message: 'one', type: 'error' });
  const b = createToast({ message: 'two', type: 'error' });
  assert.notEqual(a.id, b.id);
  assert.equal(a.type, 'error');
  assert.equal(a.message, 'one');
});

test('createToast falls back to info for unknown types', () => {
  assert.equal(createToast({ message: 'x', type: 'bogus' }).type, 'info');
  assert.equal(createToast({ message: 'x' }).type, 'info');
});

test('emitToast dispatches a toast event on the target', () => {
  const target = new EventTarget();
  let received = null;
  target.addEventListener(TOAST_EVENT, (event) => {
    received = event.detail;
  });

  assert.equal(emitToast({ message: 'Auth failed', type: 'error' }, target), true);
  assert.equal(received.message, 'Auth failed');
  assert.equal(received.type, 'error');
});

test('emitToast is a no-op without a target (SSR)', () => {
  assert.equal(emitToast({ message: 'x' }, undefined), false);
});
