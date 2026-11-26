const test = require('node:test');
const assert = require('node:assert/strict');

const {
  DEFAULT_MAX_LENGTH,
  MAX_MERMAID_LENGTH,
  escapeHtml,
  stripControlChars,
  sanitizePlainText,
  sanitizeMermaidDefinition,
  sanitizeContractId,
  isValidContractId,
} = require('./security');

test('escapeHtml escapes HTML metacharacters', () => {
  assert.equal(escapeHtml('<script>alert("xss")</script>'), '&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;');
  assert.equal(escapeHtml("it's"), 'it&#39;s');
  assert.equal(escapeHtml('a & b'), 'a &amp; b');
});

test('escapeHtml returns empty string for non-strings', () => {
  assert.equal(escapeHtml(null), '');
  assert.equal(escapeHtml(undefined), '');
  assert.equal(escapeHtml(42), '');
});

test('stripControlChars removes control characters', () => {
  assert.equal(stripControlChars('a\u0000b\u0007c\u001fd'), 'abcd');
  assert.equal(stripControlChars('hello\u000bworld'), 'helloworld');
  assert.equal(stripControlChars('plain text'), 'plain text');
  assert.equal(stripControlChars(123), '');
});

test('sanitizePlainText trims, strips control chars and enforces the length cap', () => {
  assert.equal(sanitizePlainText('  hi\u0000there  '), 'hithere');
  assert.equal(sanitizePlainText('x'.repeat(DEFAULT_MAX_LENGTH + 100)).length, DEFAULT_MAX_LENGTH);
  assert.equal(sanitizePlainText('x'.repeat(5), 3), 'xxx');
  assert.equal(sanitizePlainText(null), '');
});

test('sanitizeMermaidDefinition strips control chars and caps size', () => {
  assert.equal(sanitizeMermaidDefinition('graph LR\u0000\n  A-->B'), 'graph LR\n  A-->B');
  const big = 'a'.repeat(MAX_MERMAID_LENGTH + 500);
  assert.equal(sanitizeMermaidDefinition(big).length, MAX_MERMAID_LENGTH);
  assert.equal(sanitizeMermaidDefinition(undefined), '');
});

test('sanitizeContractId rejects blank and over-long values', () => {
  assert.equal(sanitizeContractId('  '), '');
  assert.equal(sanitizeContractId('x'.repeat(80)), '');
  assert.equal(sanitizeContractId('GAAA\u0000BBBB'), 'GAAABBBB');
  assert.equal(sanitizeContractId(null), '');
});

test('isValidContractId accepts G/C/A strkeys and rejects others', () => {
  const base32 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
  const valid = 'G' + base32.repeat(2).slice(0, 55);
  assert.equal(isValidContractId(valid), true);
  assert.equal(isValidContractId('C' + valid.slice(1)), true);
  assert.equal(isValidContractId('A' + valid.slice(1)), true);
  assert.equal(isValidContractId('short'), false);
  assert.equal(isValidContractId(valid.toLowerCase()), false);
  assert.equal(isValidContractId(valid + 'X'), false);
  assert.equal(isValidContractId('<script>'), false);
});