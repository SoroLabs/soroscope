'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  DEFAULT_COMPARE_NETWORKS,
  DIFF_THRESHOLD,
  buildComparisonRows,
  formatDifference,
  relativeDifference,
} = require('./networkComparison');

const testnet = {
  cpu_instructions: 1_000_000,
  ram_bytes: 2_000,
  ledger_read_bytes: 500,
  ledger_write_bytes: 0,
  transaction_size_bytes: 300,
  cost_stroops: 100,
};

test('compares testnet against futurenet by default with a 5% threshold', () => {
  assert.deepEqual(DEFAULT_COMPARE_NETWORKS, ['testnet', 'futurenet']);
  assert.equal(DIFF_THRESHOLD, 0.05);
});

test('relativeDifference is symmetric and handles zero and missing values', () => {
  assert.equal(relativeDifference(100, 110), relativeDifference(110, 100));
  assert.equal(relativeDifference(0, 0), 0);
  assert.equal(relativeDifference(0, 10), 1);
  assert.equal(relativeDifference(undefined, 10), null);
  assert.equal(relativeDifference('200', 200), 0);
});

test('buildComparisonRows highlights only values that differ by more than 5%', () => {
  const futurenet = {
    ...testnet,
    cpu_instructions: 1_049_000, // 4.7% — not highlighted
    ram_bytes: 2_200, // ~9.1% — highlighted
    ledger_write_bytes: 64, // from zero — highlighted
  };
  const rows = buildComparisonRows(testnet, futurenet);
  const differs = Object.fromEntries(rows.map((r) => [r.key, r.differs]));

  assert.deepEqual(differs, {
    cpu_instructions: false,
    ram_bytes: true,
    ledger_read_bytes: false,
    ledger_write_bytes: true,
    transaction_size_bytes: false,
    cost_stroops: false,
  });
});

test('a difference of exactly 5% is not highlighted', () => {
  const rows = buildComparisonRows({ cost_stroops: 100 }, { cost_stroops: 95 });
  const fee = rows.find((r) => r.key === 'cost_stroops');
  assert.ok(Math.abs(fee.difference - 0.05) < 1e-12);
  assert.equal(fee.differs, false);
});

test('a failed network leaves its column empty without highlighting', () => {
  const rows = buildComparisonRows(testnet, null);
  assert.ok(rows.every((r) => r.right === null && r.differs === false));
  assert.equal(rows[0].left, 1_000_000);
});

test('formatDifference renders percentages', () => {
  assert.equal(formatDifference(0.0912), '9.1%');
  assert.equal(formatDifference(null), '—');
});
