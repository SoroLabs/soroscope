'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  COPY_STATE,
  DEFAULT_FEEDBACK_TIMEOUT,
  createCopyFeedback,
  writeTextToClipboard,
  describeCopyError,
} = require('../copyFeedback');

/**
 * Minimal document stand-in for the execCommand fallback path.
 */
function createFakeDocument(execResult = true) {
  const created = [];
  const appended = [];

  return {
    activeElement: { focus: () => {} },
    createElement() {
      const el = {
        value: '',
        style: {},
        attributes: {},
        selected: false,
        selectionRange: null,
        setAttribute(name, value) {
          el.attributes[name] = value;
        },
        select() {
          el.selected = true;
        },
        setSelectionRange(start, end) {
          el.selectionRange = [start, end];
        },
      };
      created.push(el);
      return el;
    },
    body: {
      appendChild(el) {
        appended.push(el);
      },
      removeChild(el) {
        const index = appended.indexOf(el);
        if (index >= 0) appended.splice(index, 1);
      },
    },
    execCommand(command) {
      assert.equal(command, 'copy');
      return execResult;
    },
    created,
    appended,
  };
}

// ── copy feedback state machine ──────────────────────────────────────────────

test('createCopyFeedback: starts idle with no feedback badge', () => {
  const feedback = createCopyFeedback();
  assert.equal(feedback.state, COPY_STATE.IDLE);
  assert.equal(feedback.isIdle, true);
  assert.equal(feedback.isCopied, false);
  assert.equal(feedback.isError, false);
  feedback.dispose();
});

test('createCopyFeedback: succeed() switches to copied and notifies listeners', () => {
  const states = [];
  const feedback = createCopyFeedback({ onChange: (state) => states.push(state) });

  feedback.succeed();

  assert.equal(feedback.state, COPY_STATE.COPIED);
  assert.equal(feedback.isCopied, true);
  assert.deepEqual(states, ['copied']);
  feedback.dispose();
});

test('createCopyFeedback: fail() switches to error state', () => {
  const states = [];
  const feedback = createCopyFeedback({ onChange: (state) => states.push(state) });

  feedback.fail();

  assert.equal(feedback.state, COPY_STATE.ERROR);
  assert.equal(feedback.isError, true);
  assert.deepEqual(states, ['error']);
  feedback.dispose();
});

test('createCopyFeedback: feedback clears back to idle after the timeout', async () => {
  const feedback = createCopyFeedback({ timeout: 20 });

  feedback.succeed();
  assert.equal(feedback.state, COPY_STATE.COPIED);

  await new Promise((resolve) => setTimeout(resolve, 60));
  assert.equal(feedback.state, COPY_STATE.IDLE);
});

test('createCopyFeedback: repeated clicks restart the feedback window', async () => {
  const feedback = createCopyFeedback({ timeout: 60 });

  feedback.succeed();
  await new Promise((resolve) => setTimeout(resolve, 40));
  feedback.fail();
  assert.equal(feedback.state, COPY_STATE.ERROR);

  // The first (60ms) window would already be over at 80ms; the badge must stay.
  await new Promise((resolve) => setTimeout(resolve, 40));
  assert.equal(feedback.state, COPY_STATE.ERROR);

  await new Promise((resolve) => setTimeout(resolve, 60));
  assert.equal(feedback.state, COPY_STATE.IDLE);
});

test('createCopyFeedback: dispose() cancels the pending reset timer', () => {
  const scheduled = [];
  let cleared = 0;

  const feedback = createCopyFeedback({
    timeout: 20,
    setTimeoutFn: (handler, ms) => {
      scheduled.push({ handler, ms });
      return scheduled.length;
    },
    clearTimeoutFn: () => {
      cleared += 1;
    },
  });

  feedback.succeed();
  feedback.dispose();

  assert.equal(scheduled.length, 1, 'a reset timer was scheduled');
  assert.equal(cleared, 1, 'the pending timer was cleared on dispose');
  assert.equal(feedback.state, COPY_STATE.COPIED, 'state is frozen once disposed');
});

test('createCopyFeedback: invalid timeouts fall back to the 2s default', () => {
  assert.equal(createCopyFeedback({ timeout: 0 }).timeout, DEFAULT_FEEDBACK_TIMEOUT);
  assert.equal(createCopyFeedback({ timeout: -5 }).timeout, DEFAULT_FEEDBACK_TIMEOUT);
  assert.equal(createCopyFeedback({ timeout: NaN }).timeout, DEFAULT_FEEDBACK_TIMEOUT);
  assert.equal(DEFAULT_FEEDBACK_TIMEOUT, 2000, 'default feedback window is 2 seconds');
});

test('createCopyFeedback: duplicate transitions do not re-notify listeners', () => {
  const states = [];
  const feedback = createCopyFeedback({ onChange: (state) => states.push(state) });

  feedback.succeed();
  feedback.succeed();
  feedback.fail();
  feedback.fail();

  assert.deepEqual(states, ['copied', 'error']);
  feedback.dispose();
});

// ── clipboard writes ─────────────────────────────────────────────────────────

test('writeTextToClipboard: uses the async clipboard API when available', async () => {
  const writes = [];
  const clipboard = {
    writeText: async (value) => {
      writes.push(value);
    },
  };

  const copied = await writeTextToClipboard('CCASTELLAR...CONTRACTID', { clipboard });

  assert.equal(copied, 'CCASTELLAR...CONTRACTID');
  assert.deepEqual(writes, ['CCASTELLAR...CONTRACTID']);
});

test('writeTextToClipboard: rejects when the clipboard API throws', async () => {
  const clipboard = {
    writeText: async () => {
      const error = new Error('Write permission denied.');
      error.name = 'NotAllowedError';
      throw error;
    },
  };

  await assert.rejects(
    () => writeTextToClipboard('hash', { clipboard }),
    /permission denied/i,
  );
});

test('writeTextToClipboard: falls back to a hidden textarea without the clipboard API', async () => {
  const doc = createFakeDocument(true);

  const copied = await writeTextToClipboard('fallback text', {
    clipboard: null,
    doc,
  });

  assert.equal(copied, 'fallback text');
  assert.equal(doc.appended.length, 0, 'temporary textarea is removed again');
});

test('writeTextToClipboard: fallback marks the textarea as hidden and readonly', async () => {
  const doc = createFakeDocument(true);
  const seen = [];
  const originalCreate = doc.createElement.bind(doc);
  doc.createElement = () => {
    const el = originalCreate();
    const originalSetAttribute = el.setAttribute.bind(el);
    el.setAttribute = (name, value) => {
      seen.push([name, value]);
      originalSetAttribute(name, value);
    };
    return el;
  };

  await writeTextToClipboard('hidden', { clipboard: null, doc });

  assert.ok(seen.some(([name]) => name === 'readonly'));
  assert.ok(seen.some(([name]) => name === 'aria-hidden'));
});

test('writeTextToClipboard: surfaces a failure when execCommand rejects the copy', async () => {
  const doc = createFakeDocument(false);

  await assert.rejects(
    () => writeTextToClipboard('nope', { clipboard: null, doc }),
    /rejected by the browser/i,
  );
  assert.equal(doc.appended.length, 0, 'temporary textarea is cleaned up on failure');
});

test('writeTextToClipboard: empty text is rejected before touching the clipboard', async () => {
  let called = false;
  const clipboard = {
    writeText: async () => {
      called = true;
    },
  };

  await assert.rejects(
    () => writeTextToClipboard('', { clipboard }),
    /nothing to copy/i,
  );
  assert.equal(called, false);
});

test('writeTextToClipboard: rejects when neither clipboard nor document exist', async () => {
  await assert.rejects(
    () => writeTextToClipboard('text', { clipboard: null, doc: null }),
    /unavailable/i,
  );
});

// ── error messaging ──────────────────────────────────────────────────────────

test('describeCopyError: explains a blocked clipboard permission', () => {
  const error = new Error('Write permission denied.');
  error.name = 'NotAllowedError';
  assert.match(describeCopyError(error), /blocked by the browser/i);
});

test('describeCopyError: falls back to a generic message for unusable errors', () => {
  assert.equal(describeCopyError(undefined), 'Copy failed. Try again.');
  assert.equal(describeCopyError({}), 'Copy failed. Try again.');
  assert.equal(describeCopyError(new Error('')), 'Copy failed. Try again.');
});

test('describeCopyError: passes through meaningful messages', () => {
  assert.equal(describeCopyError('Clipboard write was rejected'), 'Clipboard write was rejected');
  assert.equal(
    describeCopyError(new Error('Nothing to copy')),
    'Nothing to copy',
  );
});
