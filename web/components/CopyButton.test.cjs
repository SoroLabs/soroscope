// CopyButton.test.cjs — unit tests for CopyButton feedback states
// Issue #835 (Client-Side: one-click code & data clipboard copy button)
// Runs with: node --test ./components/CopyButton.test.cjs

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const assert = require('node:assert/strict');

const {
  COPY_STATE,
  createCopyFeedback,
} = require('../lib/copyFeedback');

const COMPONENT_PATH = path.join(__dirname, 'CopyButton.tsx');
const TAILWIND_PATH = path.join(__dirname, '..', 'tailwind.config.js');

const componentSource = fs.readFileSync(COMPONENT_PATH, 'utf8');
const tailwindConfig = fs.readFileSync(TAILWIND_PATH, 'utf8');

const COPY_BUTTON_CALL_SITES = [
  path.join(__dirname, '..', 'pages', 'index.tsx'),
  path.join(__dirname, 'SyntaxHighlighter.tsx'),
  path.join(__dirname, 'Resultviewer.tsx'),
  path.join(__dirname, 'TransactionHistoryTable.tsx'),
];

// ── state machine ────────────────────────────────────────────────────────────

test('CopyButton: initial state is not copied', () => {
  const feedback = createCopyFeedback();

  assert.equal(feedback.state, COPY_STATE.IDLE);
  assert.equal(feedback.isCopied, false);
  assert.equal(feedback.isError, false);
  feedback.dispose();
});

test('CopyButton: triggering copy updates state to copied and shows tooltip', () => {
  const feedback = createCopyFeedback({ copiedLabel: 'Copied!' });
  let copiedText = null;

  // Mirrors the component: write the value, then flip the feedback state.
  const handleCopy = (value) => {
    copiedText = value;
    feedback.succeed();
  };

  handleCopy('CCASTELLAR...CONTRACTID');

  assert.equal(copiedText, 'CCASTELLAR...CONTRACTID');
  assert.equal(feedback.isCopied, true);
  assert.equal(feedback.state, COPY_STATE.COPIED);
  assert.match(
    componentSource,
    /showTooltip && \(isCopied \|\| isError\)/,
    'the badge renders for both success and failure',
  );

  feedback.dispose();
});

test('CopyButton: resets copied state to false after timeout (2000ms default)', async () => {
  const feedback = createCopyFeedback({ timeout: 100 });

  feedback.succeed();
  assert.equal(feedback.isCopied, true);

  await new Promise((resolve) => setTimeout(resolve, 160));

  assert.equal(feedback.isCopied, false);
  assert.equal(feedback.state, COPY_STATE.IDLE);
});

test('CopyButton: default feedback window is 2 seconds', () => {
  const feedback = createCopyFeedback();
  assert.equal(feedback.timeout, 2000);
  feedback.dispose();
});

test('CopyButton: repeated clicks cleanly reset active timer', async () => {
  const feedback = createCopyFeedback({ timeout: 100 });

  feedback.succeed();
  assert.equal(feedback.isCopied, true);

  // Second click before the first 100ms timer fires.
  await new Promise((resolve) => setTimeout(resolve, 50));
  feedback.succeed();

  // Past the original deadline: the badge must still be visible.
  await new Promise((resolve) => setTimeout(resolve, 70));
  assert.equal(feedback.isCopied, true);

  // Past the reset deadline from the second click.
  await new Promise((resolve) => setTimeout(resolve, 80));
  assert.equal(feedback.isCopied, false);
});

test('CopyButton: failure state is surfaced and clears on its own', async () => {
  const feedback = createCopyFeedback({ timeout: 60 });

  feedback.fail();
  assert.equal(feedback.isError, true);
  assert.equal(feedback.isCopied, false);

  await new Promise((resolve) => setTimeout(resolve, 120));
  assert.equal(feedback.state, COPY_STATE.IDLE);
});

test('CopyButton: a success after a failure clears the error state', () => {
  const states = [];
  const feedback = createCopyFeedback({ onChange: (state) => states.push(state) });

  feedback.fail();
  feedback.succeed();

  assert.equal(feedback.state, COPY_STATE.COPIED);
  assert.deepEqual(states, ['error', 'copied']);
  feedback.dispose();
});

// ── rendered markup / accessibility ──────────────────────────────────────────

test('CopyButton: exposes ARIA state to assistive technology', () => {
  assert.match(componentSource, /role="status"/, 'success feedback uses a live region');
  assert.match(componentSource, /aria-live="polite"/);
  assert.match(componentSource, /role="alert"/, 'failures are announced assertively');
  assert.match(componentSource, /aria-live="assertive"/);
  assert.match(componentSource, /aria-label=/, 'button keeps an accessible name');
  assert.match(componentSource, /sr-only/, 'live region is visually hidden');
  assert.match(componentSource, /aria-hidden="true"/, 'decorative icons/badges are hidden');
});

test('CopyButton: renders a checkmark on success and a warning icon on failure', () => {
  assert.match(componentSource, /Check\b/);
  assert.match(componentSource, /AlertTriangle\b/);
  assert.match(componentSource, /isCopied\s*\?/, 'success branch is conditional');
  assert.match(componentSource, /isError\s*\?/, 'failure branch is conditional');
});

test('CopyButton: success and failure feedback use distinct animations', () => {
  assert.match(componentSource, /animate-copy-check-pop/, 'checkmark pops in');
  assert.match(componentSource, /animate-copy-badge-pop/);
  assert.match(componentSource, /animate-copy-shake/, 'failure state shakes');
  assert.match(componentSource, /animate-copy-fade-in/);

  for (const animation of ['copy-fade-in', 'copy-badge-pop', 'copy-check-pop', 'copy-shake']) {
    assert.ok(
      tailwindConfig.includes(`"${animation}"`),
      `tailwind.config.js defines the ${animation} animation`,
    );
  }

  assert.match(tailwindConfig, /keyframes:/);
  assert.match(tailwindConfig, /animation:/);
});

test('CopyButton: animations respect reduced-motion preferences', () => {
  assert.match(componentSource, /motion-safe:animate-/, 'animation is gated behind motion-safe');
});

test('CopyButton: exposes the current state for tests and styling', () => {
  assert.match(componentSource, /data-state=\{status\}/);
  assert.match(componentSource, /data-copy-state=\{status\}/);
});

test('CopyButton: button touch target height minimum standards', () => {
  assert.match(componentSource, /min-h-\[36px\]/, '36px minimum touch target');
});

// ── integration with call sites ──────────────────────────────────────────────

test('CopyButton: every call site passes the text prop', () => {
  for (const callSite of COPY_BUTTON_CALL_SITES) {
    const source = fs.readFileSync(callSite, 'utf8');
    const usages = source.match(/<CopyButton[\s\S]*?\/>/g) || [];

    assert.ok(usages.length > 0, `${path.basename(callSite)} renders a CopyButton`);

    for (const usage of usages) {
      assert.match(
        usage,
        /\btext=/,
        `${path.basename(callSite)} passes text= to CopyButton (got: ${usage.replace(/\s+/g, ' ')})`,
      );
    }
  }
});
