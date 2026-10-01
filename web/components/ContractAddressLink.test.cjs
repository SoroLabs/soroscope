// ContractAddressLink.test.cjs — unit tests for the explorer link component contract
// Closes Issue #842
// Runs with: node --test ./components/ContractAddressLink.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildAccountExplorerUrl,
  buildContractExplorerUrl,
  buildTxExplorerUrl,
  EXPLORER_PROVIDERS,
  isLinkableIdentifier,
  SAFE_EXTERNAL_LINK_PROPS,
  shortenAddress,
} = require('../lib/explorerLinks');

const CONTRACT_ID = 'CC3JXQKZC3J4K2L9M8N7P6Q5R4S3T2U1V0W9X8Y7Z6A5B4C3D2E1F4KL9';

/**
 * Mirrors the render decision in ContractAddressLink.tsx: the component only
 * emits an anchor when the value is linkable and the network has an explorer.
 */
function renderLink({ value, kind = 'contract', networkId, provider = EXPLORER_PROVIDERS.stellarExpert }) {
  const displayText = shortenAddress(value);

  if (!isLinkableIdentifier(value)) {
    return { tag: 'span', displayText, href: null, rel: null, target: null };
  }

  const href =
    kind === 'account'
      ? buildAccountExplorerUrl(networkId, value, provider)
      : kind === 'tx'
        ? buildTxExplorerUrl(networkId, value, provider)
        : buildContractExplorerUrl(networkId, value, provider);

  if (!href) {
    return { tag: 'span', displayText, href: null, rel: null, target: null };
  }

  return {
    tag: 'a',
    displayText,
    href,
    target: SAFE_EXTERNAL_LINK_PROPS.target,
    rel: SAFE_EXTERNAL_LINK_PROPS.rel,
  };
}

test('renders the shortened address string', () => {
  const link = renderLink({ value: CONTRACT_ID, networkId: 'testnet' });
  assert.equal(link.displayText, 'CC3J...4KL9');
  assert.ok(!link.displayText.includes('...') === false);
  assert.ok(link.displayText.length < CONTRACT_ID.length);
});

test('links to Stellar Expert for the active network', () => {
  assert.equal(
    renderLink({ value: CONTRACT_ID, networkId: 'testnet' }).href,
    `https://stellar.expert/explorer/testnet/contract/${CONTRACT_ID}`,
  );
  assert.equal(
    renderLink({ value: CONTRACT_ID, networkId: 'mainnet' }).href,
    `https://stellar.expert/explorer/public/contract/${CONTRACT_ID}`,
  );
  assert.equal(
    renderLink({ value: CONTRACT_ID, networkId: 'futurenet' }).href,
    `https://stellar.expert/explorer/futurenet/contract/${CONTRACT_ID}`,
  );
});

test('links to the Soroban explorer when that provider is selected', () => {
  assert.equal(
    renderLink({ value: CONTRACT_ID, networkId: 'testnet', provider: EXPLORER_PROVIDERS.soroban }).href,
    `https://soroban-testnet.stellar.org/contract/${CONTRACT_ID}`,
  );
  assert.equal(
    renderLink({ value: CONTRACT_ID, networkId: 'mainnet', provider: EXPLORER_PROVIDERS.soroban }).href,
    `https://soroban.stellar.org/contract/${CONTRACT_ID}`,
  );
});

test('the target URL follows the network when it changes', () => {
  const urls = ['mainnet', 'testnet', 'futurenet'].map((networkId) =>
    renderLink({ value: CONTRACT_ID, networkId }).href,
  );
  assert.equal(new Set(urls).size, 3, 'each network must produce a distinct URL');
  assert.ok(urls.every((url) => url.startsWith('https://stellar.expert/explorer/')));
});

test('opens in a new tab with rel="noopener noreferrer"', () => {
  const link = renderLink({ value: CONTRACT_ID, networkId: 'testnet' });
  assert.equal(link.target, '_blank');
  assert.equal(link.rel, 'noopener noreferrer');
});

test('supports account and transaction kinds', () => {
  assert.equal(
    renderLink({ value: CONTRACT_ID, kind: 'account', networkId: 'testnet' }).href,
    `https://stellar.expert/explorer/testnet/account/${CONTRACT_ID}`,
  );
  assert.equal(
    renderLink({ value: CONTRACT_ID, kind: 'tx', networkId: 'testnet' }).href,
    `https://stellar.expert/explorer/testnet/tx/${CONTRACT_ID}`,
  );
});

test('falls back to plain text instead of rendering a dead link', () => {
  for (const value of ['', '   ', 'CC 3J', null, undefined]) {
    const link = renderLink({ value, networkId: 'testnet' });
    assert.equal(link.tag, 'span', `value ${JSON.stringify(value)} must not be linked`);
    assert.equal(link.href, null);
  }
});

test('does not link when the network has no public explorer', () => {
  const link = renderLink({ value: CONTRACT_ID, networkId: 'localhost' });
  assert.equal(link.tag, 'span');
  assert.equal(link.href, null);
});
