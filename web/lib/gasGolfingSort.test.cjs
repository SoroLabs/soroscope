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


// ── #1285: rows are keyed on a unique, stable id ─────────────────────────────
const { normalizeGasGolfingSuggestions } = require('./gasGolfingSort');

const serverSuggestions = [
  {
    id: 'loop_optimization@run#+3',
    pattern_type: 'loop_optimization',
    description: 'call inside loop',
    location: 'run#+3',
    severity: 'high',
    gas_saved_estimate: null,
    suggested_fix: 'Hoist the call',
    code_example: null,
  },
  {
    id: 'memory_allocation@run#+9',
    pattern_type: 'memory_allocation',
    description: 'memory.grow',
    location: 'run#+9',
    severity: 'medium',
    gas_saved_estimate: 120,
    suggested_fix: 'Preallocate',
    code_example: 'Vec::with_capacity(n)',
  },
];

test('normalizeGasGolfingSuggestions: keeps server ids and maps fields for the table', () => {
  const rows = normalizeGasGolfingSuggestions(serverSuggestions);
  assert.deepEqual(rows.map((r) => r.id), ['loop_optimization@run#+3', 'memory_allocation@run#+9']);
  assert.equal(rows[0].title, 'Loop optimization');
  assert.equal(rows[0].recommendation, 'Hoist the call');
  assert.equal(rows[1].gas_saved_estimate, 120);
});

test('normalizeGasGolfingSuggestions: derives unique ids when the server omits them', () => {
  const legacy = serverSuggestions.map(({ id, ...rest }) => rest);
  const rows = normalizeGasGolfingSuggestions([...legacy, legacy[0]]);
  const ids = rows.map((r) => r.id);
  assert.equal(new Set(ids).size, ids.length, `duplicate ids: ${ids}`);
  assert.deepEqual(ids, [
    'loop_optimization@run#+3',
    'memory_allocation@run#+9',
    'loop_optimization@run#+3#2',
  ]);
});

test('normalizeGasGolfingSuggestions: is idempotent', () => {
  const once = normalizeGasGolfingSuggestions(serverSuggestions);
  assert.deepEqual(normalizeGasGolfingSuggestions(once), once);
});

test('normalizeGasGolfingSuggestions: ids follow rows through re-sorts', () => {
  const rows = normalizeGasGolfingSuggestions(serverSuggestions);
  const desc = sortGasGolfingSuggestions(rows, 'gas_saved_estimate', 'desc');
  const asc = sortGasGolfingSuggestions(rows, 'gas_saved_estimate', 'asc');
  const byId = (list) => Object.fromEntries(list.map((r) => [r.id, r.title]));
  assert.deepEqual(byId(desc), byId(asc));
  assert.notDeepEqual(desc.map((r) => r.id), asc.map((r) => r.id));
});

test('normalizeGasGolfingSuggestions: tolerates bad input', () => {
  assert.deepEqual(normalizeGasGolfingSuggestions(undefined), []);
  assert.deepEqual(normalizeGasGolfingSuggestions([null, 3]), []);
});
