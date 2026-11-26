// searchCommands.test.cjs — unit tests for the Cmd+K global search overlay
// Closes Issue #623
// Runs with: node --test ./lib/searchCommands.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  BASE_COMMANDS,
  isSearchShortcut,
  isDismissShortcut,
  scoreCommand,
  filterCommands,
  buildCommandRegistry,
  moveHighlight,
} = require('./searchCommands');

// ── Shortcut detection ──────────────────────────────────────────────────────

test('isSearchShortcut: matches Ctrl+K and Cmd+K', () => {
  assert.equal(isSearchShortcut({ key: 'k', ctrlKey: true }), true);
  assert.equal(isSearchShortcut({ key: 'k', metaKey: true }), true);
});

test('isSearchShortcut: is case insensitive (Shift+Cmd+K still opens)', () => {
  assert.equal(isSearchShortcut({ key: 'K', metaKey: true, shiftKey: true }), true);
});

test('isSearchShortcut: ignores a bare k and other modifier combos', () => {
  assert.equal(isSearchShortcut({ key: 'k' }), false);
  assert.equal(isSearchShortcut({ key: 'k', altKey: true, metaKey: true }), false);
  assert.equal(isSearchShortcut({ key: 'j', ctrlKey: true }), false);
});

test('isSearchShortcut: tolerates missing or malformed events', () => {
  assert.equal(isSearchShortcut(null), false);
  assert.equal(isSearchShortcut(undefined), false);
  assert.equal(isSearchShortcut({}), false);
});

test('isDismissShortcut: matches Escape only', () => {
  assert.equal(isDismissShortcut({ key: 'Escape' }), true);
  assert.equal(isDismissShortcut({ key: 'Enter' }), false);
  assert.equal(isDismissShortcut(null), false);
});

// ── Registry ────────────────────────────────────────────────────────────────

test('buildCommandRegistry: includes the base navigation commands', () => {
  const registry = buildCommandRegistry();
  assert.equal(registry.length, BASE_COMMANDS.length);
  assert.ok(registry.some((c) => c.href === '/settings'));
});

test('buildCommandRegistry: appends contract functions as selectable commands', () => {
  const registry = buildCommandRegistry({ functions: [{ name: 'transfer' }, { name: 'mint' }] });
  const transfer = registry.find((c) => c.id === 'fn-transfer');
  assert.ok(transfer);
  assert.equal(transfer.action, 'select-function');
  assert.deepEqual(transfer.payload, { name: 'transfer' });
});

test('buildCommandRegistry: every command has a unique id', () => {
  const ids = buildCommandRegistry({ functions: [{ name: 'transfer' }] }).map((c) => c.id);
  assert.equal(new Set(ids).size, ids.length);
});

// ── Scoring & filtering ─────────────────────────────────────────────────────

const COMMANDS = [
  { id: 'a', title: 'Settings', subtitle: 'Custom RPC endpoints', group: 'Preferences', keywords: ['rpc', 'indexer'] },
  { id: 'b', title: 'Invocation history', subtitle: 'Previous runs', group: 'Navigation', keywords: ['past'] },
  { id: 'c', title: 'Transactions', subtitle: 'Recent on-chain activity', group: 'Navigation', keywords: ['tx'] },
];

test('scoreCommand: an exact title beats a prefix which beats a substring', () => {
  const exact = scoreCommand(COMMANDS[0], 'settings');
  const prefix = scoreCommand(COMMANDS[0], 'sett');
  const subtitleHit = scoreCommand(COMMANDS[0], 'endpoints');
  assert.ok(exact > prefix);
  assert.ok(prefix > subtitleHit);
});

test('scoreCommand: returns 0 when nothing matches', () => {
  assert.equal(scoreCommand(COMMANDS[0], 'zzzzzz'), 0);
});

test('filterCommands: an empty query returns everything in registry order', () => {
  assert.deepEqual(
    filterCommands(COMMANDS, '').map((c) => c.id),
    ['a', 'b', 'c'],
  );
  assert.deepEqual(filterCommands(COMMANDS, '   ').length, 3);
});

test('filterCommands: matching is case insensitive', () => {
  assert.deepEqual(
    filterCommands(COMMANDS, 'SETTINGS').map((c) => c.id),
    ['a'],
  );
});

test('filterCommands: matches on keywords as well as titles', () => {
  assert.deepEqual(
    filterCommands(COMMANDS, 'rpc').map((c) => c.id),
    ['a'],
  );
});

test('filterCommands: matches a word start inside a multi-word title', () => {
  assert.deepEqual(
    filterCommands(COMMANDS, 'history').map((c) => c.id),
    ['b'],
  );
});

test('filterCommands: ranks the strongest match first', () => {
  const results = filterCommands(COMMANDS, 'tran');
  assert.equal(results[0].id, 'c');
});

test('filterCommands: returns an empty list when nothing matches', () => {
  assert.deepEqual(filterCommands(COMMANDS, 'quantum'), []);
});

test('filterCommands: honours the result limit', () => {
  assert.equal(filterCommands(COMMANDS, '', { limit: 2 }).length, 2);
});

test('filterCommands: tolerates a non-array registry', () => {
  assert.deepEqual(filterCommands(null, 'x'), []);
  assert.deepEqual(filterCommands(undefined, ''), []);
});

// ── Keyboard navigation ─────────────────────────────────────────────────────

test('moveHighlight: steps forward and wraps at the end', () => {
  assert.equal(moveHighlight(0, 1, 3), 1);
  assert.equal(moveHighlight(2, 1, 3), 0);
});

test('moveHighlight: steps backward and wraps at the start', () => {
  assert.equal(moveHighlight(1, -1, 3), 0);
  assert.equal(moveHighlight(0, -1, 3), 2);
});

test('moveHighlight: stays at 0 for an empty result list', () => {
  assert.equal(moveHighlight(0, 1, 0), 0);
  assert.equal(moveHighlight(3, -1, 0), 0);
});
