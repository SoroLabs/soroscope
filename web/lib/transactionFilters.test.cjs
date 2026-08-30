// transactionFilters.test.cjs — unit tests for event log table filtering logic
// Runs with: node --test ./lib/transactionFilters.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// ── Filtering logic (duplicated from lib/transactionFilters.ts for isolated testing) ──

function filterTransactions(transactions, filter) {
  const query = filter.functionName.trim().toLowerCase();

  return transactions.filter((tx) => {
    const matchesStatus = filter.status === 'all' || tx.status === filter.status;
    const matchesFunction = query.length === 0 || tx.functionName.toLowerCase().includes(query);
    return matchesStatus && matchesFunction;
  });
}

function createMockTransaction(overrides = {}) {
  return {
    hash: overrides.hash ?? 'a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b',
    functionName: overrides.functionName ?? 'transfer',
    status: overrides.status ?? 'success',
    timestamp: overrides.timestamp ?? Date.now(),
    contractId: overrides.contractId ?? 'CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q',
    fee: overrides.fee ?? '0.00123',
  };
}

const SAMPLE = [
  createMockTransaction({ hash: 'tx1', functionName: 'transfer', status: 'success' }),
  createMockTransaction({ hash: 'tx2', functionName: 'swap', status: 'failed' }),
  createMockTransaction({ hash: 'tx3', functionName: 'mint', status: 'pending' }),
  createMockTransaction({ hash: 'tx4', functionName: 'Transfer_Batch', status: 'success' }),
];

test('transactionFilters: "all" status with empty query returns every transaction', () => {
  const result = filterTransactions(SAMPLE, { status: 'all', functionName: '' });
  assert.equal(result.length, 4);
});

test('transactionFilters: filters by exact status', () => {
  const result = filterTransactions(SAMPLE, { status: 'failed', functionName: '' });
  assert.equal(result.length, 1);
  assert.equal(result[0].hash, 'tx2');
});

test('transactionFilters: filters by function name, case-insensitively', () => {
  const result = filterTransactions(SAMPLE, { status: 'all', functionName: 'TRANSFER' });
  assert.equal(result.length, 2);
  assert.deepEqual(result.map((tx) => tx.hash).sort(), ['tx1', 'tx4']);
});

test('transactionFilters: trims whitespace from the function name query', () => {
  const result = filterTransactions(SAMPLE, { status: 'all', functionName: '  swap  ' });
  assert.equal(result.length, 1);
  assert.equal(result[0].hash, 'tx2');
});

test('transactionFilters: combines status and function name filters', () => {
  const result = filterTransactions(SAMPLE, { status: 'success', functionName: 'transfer' });
  assert.equal(result.length, 2);
});

test('transactionFilters: returns an empty array when nothing matches', () => {
  const result = filterTransactions(SAMPLE, { status: 'pending', functionName: 'swap' });
  assert.equal(result.length, 0);
});
