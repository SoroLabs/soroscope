'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { buildTransactionToast, getTransactionToastTitle, getTransactionToastMessage } = require('./transactionToasts');

test('buildTransactionToast creates a signing toast with the expected title', () => {
  const toast = buildTransactionToast('signing', { message: 'Please review and approve the transaction.' });
  assert.equal(toast.phase, 'signing');
  assert.equal(getTransactionToastTitle('signing'), 'Signing...');
  assert.equal(toast.title, 'Signing...');
  assert.match(toast.message, /Please review and approve/);
});

test('buildTransactionToast includes a tx hash for successful transactions', () => {
  const toast = buildTransactionToast('success', { txHash: '0xabc123' });
  assert.equal(toast.phase, 'success');
  assert.equal(toast.title, 'Success');
  assert.match(toast.message, /0xabc123/);
});

test('buildTransactionToast formats failed toasts using the provided error', () => {
  const toast = buildTransactionToast('failed', { message: 'The transaction was rejected.' });
  assert.equal(toast.title, 'Failed');
  assert.equal(toast.message, 'The transaction was rejected.');
  assert.equal(getTransactionToastMessage('failed', { message: 'The transaction was rejected.' }), 'The transaction was rejected.');
});
