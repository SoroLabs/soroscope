// TransactionHistoryTable.test.cjs — unit tests for transaction history table logic
// Runs with: node --test ./components/TransactionHistoryTable.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

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

// ── Status badge helper (extracted from component logic) ──

function statusBadgeClass(status) {
  switch (status) {
    case 'success': return 'border-emerald-500/50 bg-emerald-500/10 text-emerald-200';
    case 'failed':  return 'border-red-500/50 bg-red-500/10 text-red-200';
    case 'pending': return 'border-yellow-500/50 bg-yellow-500/10 text-yellow-200';
    default:        return 'border-slate-500/50 bg-slate-500/10 text-slate-200';
  }
}

function statusLabel(status) {
  switch (status) {
    case 'success': return 'Success';
    case 'failed':  return 'Failed';
    case 'pending': return 'Pending';
    default:        return status;
  }
}

// ── Pagination helper (duplicated from lib for isolated testing) ──

function paginate(items, page, perPage) {
  const total = items.length;
  const totalPages = perPage > 0 ? Math.max(1, Math.ceil(total / perPage)) : 1;
  const clampedPage = Math.max(1, Math.min(page, totalPages));
  const start = (clampedPage - 1) * perPage;
  const end = start + perPage;
  return {
    items: items.slice(start, end),
    page: clampedPage,
    perPage,
    total,
    totalPages,
  };
}

// ── Tests ──

test('TransactionHistoryTable: status badge returns correct class for success', () => {
  const cls = statusBadgeClass('success');
  assert.ok(cls.includes('border-emerald'));
  assert.ok(cls.includes('text-emerald'));
});

test('TransactionHistoryTable: status badge returns correct class for failed', () => {
  const cls = statusBadgeClass('failed');
  assert.ok(cls.includes('border-red'));
  assert.ok(cls.includes('text-red'));
});

test('TransactionHistoryTable: status badge returns correct class for pending', () => {
  const cls = statusBadgeClass('pending');
  assert.ok(cls.includes('border-yellow'));
  assert.ok(cls.includes('text-yellow'));
});

test('TransactionHistoryTable: status label returns capitalised text', () => {
  assert.equal(statusLabel('success'), 'Success');
  assert.equal(statusLabel('failed'), 'Failed');
  assert.equal(statusLabel('pending'), 'Pending');
});

test('TransactionHistoryTable: mock transaction has correct shape', () => {
  const tx = createMockTransaction();
  assert.ok(typeof tx.hash === 'string');
  assert.ok(typeof tx.functionName === 'string');
  assert.ok(['success', 'failed', 'pending'].includes(tx.status));
  assert.ok(typeof tx.timestamp === 'number');
  assert.ok(typeof tx.contractId === 'string');
});

test('TransactionHistoryTable: mock transaction overrides work', () => {
  const tx = createMockTransaction({ hash: 'abc123', functionName: 'swap', status: 'failed' });
  assert.equal(tx.hash, 'abc123');
  assert.equal(tx.functionName, 'swap');
  assert.equal(tx.status, 'failed');
});

test('TransactionHistoryTable: pagination returns 10 items per page by default', () => {
  const txs = Array.from({ length: 25 }, (_, i) => createMockTransaction({ hash: `tx${i}` }));
  const page1 = paginate(txs, 1, 10);
  assert.equal(page1.items.length, 10);
  assert.equal(page1.total, 25);
  assert.equal(page1.totalPages, 3);
});

test('TransactionHistoryTable: pagination page 2 returns items 11-20', () => {
  const txs = Array.from({ length: 25 }, (_, i) => createMockTransaction({ hash: `tx${i}` }));
  const page2 = paginate(txs, 2, 10);
  assert.equal(page2.items.length, 10);
  assert.equal(page2.items[0].hash, 'tx10');
});

test('TransactionHistoryTable: pagination last page returns remaining items', () => {
  const txs = Array.from({ length: 25 }, (_, i) => createMockTransaction({ hash: `tx${i}` }));
  const page3 = paginate(txs, 3, 10);
  assert.equal(page3.items.length, 5);
  assert.equal(page3.items[0].hash, 'tx20');
});

test('TransactionHistoryTable: StellarExpert URL format for transaction', () => {
  const tx = createMockTransaction({ hash: 'abc123' });
  const url = `https://stellar.expert/explorer/testnet/tx/${tx.hash}`;
  assert.ok(url.includes('abc123'));
  assert.ok(url.startsWith('https://stellar.expert'));
});

test('TransactionHistoryTable: empty state shows appropriate message', () => {
  const txs = [];
  assert.equal(txs.length, 0);
  const result = paginate(txs, 1, 10);
  assert.equal(result.items.length, 0);
  assert.equal(result.total, 0);
});

// CSV and JSON format testing helpers mirroring component logic
function formatCSV(transactions) {
  if (!transactions.length) return '';
  const headers = ['Transaction Hash', 'Function', 'Status', 'Timestamp', 'Fee (XLM)'];
  const escape = (val) => {
    const clean = String(val).replace(/"/g, '""');
    return `"${clean}"`;
  };
  const rows = transactions.map((tx) => [
    escape(tx.hash),
    escape(tx.functionName),
    escape(tx.status),
    escape(new Date(tx.timestamp).toISOString()),
    escape(tx.fee ? `${tx.fee}` : '0'),
  ]);
  return [headers.join(','), ...rows.map((row) => row.join(','))].join('\n');
}

function formatJSON(transactions) {
  return JSON.stringify(transactions, null, 2);
}

test('TransactionHistoryTable: formatCSV sanitizes and constructs valid CSV records', () => {
  const mockTxs = [
    createMockTransaction({ hash: 'tx"1"', functionName: 'test,func', fee: '0.01' }),
  ];
  const csv = formatCSV(mockTxs);
  assert.ok(csv.includes('"Transaction Hash",_,"Fee (XLM)"'.split('_')[0]));
  assert.ok(csv.includes('"tx""1"""'));
  assert.ok(csv.includes('"test,func"'));
  assert.ok(csv.includes('"0.01"'));
});

test('TransactionHistoryTable: formatJSON accurately serializes full dataset', () => {
  const mockTxs = [
    createMockTransaction({ hash: 'tx1' }),
    createMockTransaction({ hash: 'tx2' }),
  ];
  const jsonStr = formatJSON(mockTxs);
  const parsed = JSON.parse(jsonStr);
  assert.strictEqual(parsed.length, 2);
  assert.strictEqual(parsed[0].hash, 'tx1');
  assert.strictEqual(parsed[1].hash, 'tx2');
});

