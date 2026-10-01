'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {
  DEFAULT_TOAST_DURATION_MS,
  MAX_VISIBLE_TOASTS,
  SWIPE_DISMISS_THRESHOLD_PX,
  dismissToast,
  enqueueToast,
  getVisibleToasts,
} = require('../lib/toastQueue.cjs');

const toast = (id) => ({ id, message: `Toast ${id}`, type: 'info' });

test('toast notifications queue in insertion order and dismiss by id', () => {
  const queue = enqueueToast(enqueueToast([], toast('first')), toast('second'));

  assert.deepEqual(queue.map(({ id }) => id), ['first', 'second']);
  assert.deepEqual(dismissToast(queue, 'first').map(({ id }) => id), ['second']);
});

test('toast stack shows oldest items first and reveals queued alerts after dismissal', () => {
  const queue = Array.from({ length: MAX_VISIBLE_TOASTS + 2 }, (_, index) => toast(`${index + 1}`));
  const visible = getVisibleToasts(queue);
  const afterDismissal = dismissToast(queue, visible[0].id);

  assert.equal(visible.length, MAX_VISIBLE_TOASTS);
  assert.deepEqual(visible.map(({ id }) => id), ['1', '2', '3', '4', '5']);
  assert.equal(getVisibleToasts(afterDismissal).at(-1).id, '6');
});

test('toast defaults to four seconds and dismisses on a horizontal touch swipe', () => {
  const component = fs.readFileSync(path.join(__dirname, 'Toast.tsx'), 'utf8');

  assert.equal(DEFAULT_TOAST_DURATION_MS, 4000);
  assert.equal(SWIPE_DISMISS_THRESHOLD_PX, 60);
  assert.match(component, /onTouchStart/);
  assert.match(component, /onTouchEnd/);
  assert.match(component, /transition: `opacity/);
  assert.match(component, /Math\.max\(0, duration - EXIT_DURATION_MS\)/);
});

test('toast stack supports all four notification severities', () => {
  const component = fs.readFileSync(path.join(__dirname, 'Toast.tsx'), 'utf8');

  for (const type of ['error', 'success', 'warning', 'info']) {
    assert.match(component, new RegExp(`${type}: \\{`));
  }
});
