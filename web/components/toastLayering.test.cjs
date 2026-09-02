'use strict';

const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const assert = require('node:assert/strict');

function readComponent(fileName) {
  return fs.readFileSync(path.join(__dirname, fileName), 'utf8');
}

function extractZIndex(contents, pattern) {
  const match = contents.match(pattern);
  if (!match) {
    throw new Error(`Could not find z-index in pattern: ${pattern}`);
  }

  const zValue = match[1];
  return Number.parseInt(zValue, 10) || 0;
}

test('toast layering stays above the top navigation', () => {
  const headerContents = readComponent('HeaderNav.tsx');
  const toastContents = readComponent('Toast.tsx');
  const copyButtonContents = readComponent('CopyButton.tsx');

  const headerZ = extractZIndex(headerContents, /className="sticky top-0 z-?\[?(\d+)\]?/);
  const toastZ = extractZIndex(toastContents, /zIndex:\s*(\d+)/);
  const copyToastZ = extractZIndex(copyButtonContents, /absolute z-?\[?(\d+)\]?/);

  assert.ok(headerZ < toastZ, 'toast z-index must be above the header');
  assert.ok(copyToastZ >= toastZ, 'copy feedback should not render behind the toast');
  assert.ok(copyToastZ > headerZ, 'copy feedback must remain above the header');
});
