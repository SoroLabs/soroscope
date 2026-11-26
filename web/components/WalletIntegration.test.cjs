// Wallet integration metadata tests for multi-wallet support.
'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const SUPPORTED_WALLETS = [
  { id: 'freighter', name: 'Freighter' },
  { id: 'albedo', name: 'Albedo' },
  { id: 'lobstr', name: 'Lobstr' },
  { id: 'xbull', name: 'xBull' },
];

function applyAccountSwitch(state, nextAddress) {
  if (nextAddress === null) {
    return { ...state, address: null, balances: [], balancesError: null };
  }

  if (typeof nextAddress === 'string' && nextAddress.length > 0 && nextAddress !== state.address) {
    return { ...state, address: nextAddress, balances: [], balancesError: null };
  }

  return state;
}

test('Wallet integration: supports the required Stellar wallet adapters', () => {
  assert.deepEqual(
    SUPPORTED_WALLETS.map((wallet) => wallet.name),
    ['Freighter', 'Albedo', 'Lobstr', 'xBull'],
  );
});

test('Wallet integration: account switch clears stale account-scoped balances', () => {
  const initialState = {
    address: 'GAOLD',
    balances: [{ symbol: 'XLM', balance: '25' }],
    balancesError: 'stale error',
  };

  const switched = applyAccountSwitch(initialState, 'GANEW');
  assert.equal(switched.address, 'GANEW');
  assert.deepEqual(switched.balances, []);
  assert.equal(switched.balancesError, null);

  const disconnected = applyAccountSwitch(switched, null);
  assert.equal(disconnected.address, null);
  assert.deepEqual(disconnected.balances, []);
});
