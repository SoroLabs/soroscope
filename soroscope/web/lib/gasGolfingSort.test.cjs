const test = require('node:test');
const assert = require('node:assert/strict');

const { sortGasGolfingSuggestions } = require('./gasGolfingSort');

test('sortGasGolfingSuggestions: severity desc (high > medium > low)', () => {
  const sorted = sortGasGolfingSuggestions(
    [
      { title: 'a', severity: 'low', gas_saved_estimate: 10 },
      { title: 'b', severity: 'high', gas_saved_estimate: 1 },
      { title: 'c', severity: 'medium', gas_saved_estimate: 999 },
    ],
    'severity',
    'desc',
  );
  assert.deepEqual(sorted.map((s) => s.title), ['b', 'c', 'a']);
});

test('sortGasGolfingSuggestions: gas_saved_estimate asc', () => {
  const sorted = sortGasGolfingSuggestions(
    [
      { title: 'a', severity: 'low', gas_saved_estimate: 50 },
      { title: 'b', severity: 'high', gas_saved_estimate: 10 },
      { title: 'c', severity: 'medium', gas_saved_estimate: 20 },
    ],
    'gas_saved_estimate',
    'asc',
  );
  assert.deepEqual(sorted.map((s) => s.title), ['b', 'c', 'a']);
});

test('sortGasGolfingSuggestions: missing gas_saved_estimate sorts last for gas_saved_estimate desc', () => {
  const sorted = sortGasGolfingSuggestions(
    [
      { title: 'missing', severity: 'high', gas_saved_estimate: null },
      { title: 'present', severity: 'low', gas_saved_estimate: 1 },
    ],
    'gas_saved_estimate',
    'desc',
  );
  assert.deepEqual(sorted.map((s) => s.title), ['present', 'missing']);
});

