// explorerLinks.test.cjs — unit tests for Stellar explorer URL/shortening helpers
// Closes Issue #842
// Runs with: node --test ./lib/explorerLinks.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  SHORTEN_PREFIX_LENGTH,
  SHORTEN_SUFFIX_LENGTH,
  STELLAR_EXPERT_NETWORK_SEGMENTS,
  SOROBAN_EXPLORER_ORIGINS,
  EXPLORER_PROVIDERS,
  SAFE_EXTERNAL_LINK_PROPS,
  resolveNetworkId,
  shortenAddress,
  isLinkableIdentifier,
  encodeIdentifier,
  getExplorerBaseUrl,
  buildExplorerUrl,
  buildContractExplorerUrl,
  buildAccountExplorerUrl,
  buildTxExplorerUrl,
} = require('./explorerLinks');

const CONTRACT_ID = 'CC3JXQKZC3J4K2L9M8N7P6Q5R4S3T2U1V0W9X8Y7Z6A5B4C3D2E1F4KL9';
const TX_HASH = 'a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90';

test('shortenAddress renders the documented CC3J...4KL9 form', () => {
  assert.equal(shortenAddress(CONTRACT_ID), 'CC3J...4KL9');
});

test('shortenAddress uses 4-char prefix/suffix by default', () => {
  assert.equal(SHORTEN_PREFIX_LENGTH, 4);
  assert.equal(SHORTEN_SUFFIX_LENGTH, 4);
});

test('shortenAddress trims surrounding whitespace and handles non-strings', () => {
  assert.equal(shortenAddress(`  ${CONTRACT_ID}  `), 'CC3J...4KL9');
  assert.equal(shortenAddress(null), '');
  assert.equal(shortenAddress(undefined), '');
  assert.equal(shortenAddress(42), '');
  assert.equal(shortenAddress('   '), '');
});

test('shortenAddress leaves values that are already short untouched', () => {
  assert.equal(shortenAddress('CC3J...4KL9'), 'CC3J...4KL9');
  assert.equal(shortenAddress('short'), 'short');
  assert.equal(shortenAddress('12345678'), '12345678');
});

test('shortenAddress honours custom prefix/suffix lengths', () => {
  assert.equal(shortenAddress(CONTRACT_ID, { prefixLength: 6, suffixLength: 2 }), 'CC3JXQ...L9');
  assert.equal(shortenAddress(CONTRACT_ID, { separator: '…' }), 'CC3J…4KL9');
});

test('isLinkableIdentifier rejects blank, non-string and whitespace values', () => {
  assert.equal(isLinkableIdentifier(CONTRACT_ID), true);
  assert.equal(isLinkableIdentifier('  '), false);
  assert.equal(isLinkableIdentifier(''), false);
  assert.equal(isLinkableIdentifier('CC 3J'), false);
  assert.equal(isLinkableIdentifier(null), false);
  assert.equal(isLinkableIdentifier(12345), false);
});

test('encodeIdentifier percent-encodes path-breaking characters', () => {
  assert.equal(encodeIdentifier('a/b?c#d'), 'a%2Fb%3Fc%23d');
  assert.equal(encodeIdentifier(' CC3J '), 'CC3J');
  assert.equal(encodeIdentifier(null), '');
});

test('resolveNetworkId normalizes ids and defaults to testnet', () => {
  assert.equal(resolveNetworkId('mainnet'), 'mainnet');
  assert.equal(resolveNetworkId('  MAINNET '), 'mainnet');
  assert.equal(resolveNetworkId('public'), 'mainnet');
  assert.equal(resolveNetworkId('testnet'), 'testnet');
  assert.equal(resolveNetworkId('futurenet'), 'futurenet');
  assert.equal(resolveNetworkId('localhost'), 'localhost');
  assert.equal(resolveNetworkId('bogus'), 'testnet');
  assert.equal(resolveNetworkId(undefined), 'testnet');
  assert.equal(resolveNetworkId(null), 'testnet');
});

test('getExplorerBaseUrl maps each network to a Stellar Expert base URL', () => {
  assert.equal(getExplorerBaseUrl('mainnet'), 'https://stellar.expert/explorer/public');
  assert.equal(getExplorerBaseUrl('testnet'), 'https://stellar.expert/explorer/testnet');
  assert.equal(getExplorerBaseUrl('futurenet'), 'https://stellar.expert/explorer/futurenet');
});

test('getExplorerBaseUrl exposes mainnet as `public` for stellar.expert', () => {
  assert.equal(STELLAR_EXPERT_NETWORK_SEGMENTS.mainnet, 'public');
});

test('getExplorerBaseUrl maps each network to a Soroban explorer origin', () => {
  assert.equal(getExplorerBaseUrl('mainnet', EXPLORER_PROVIDERS.soroban), 'https://soroban.stellar.org');
  assert.equal(getExplorerBaseUrl('testnet', EXPLORER_PROVIDERS.soroban), 'https://soroban-testnet.stellar.org');
  assert.equal(getExplorerBaseUrl('futurenet', EXPLORER_PROVIDERS.soroban), 'https://soroban-futurenet.stellar.org');
  assert.equal(SOROBAN_EXPLORER_ORIGINS.testnet, 'https://soroban-testnet.stellar.org');
});

test('getExplorerBaseUrl returns null for networks without a public explorer', () => {
  assert.equal(getExplorerBaseUrl('localhost'), null);
  assert.equal(getExplorerBaseUrl('localhost', EXPLORER_PROVIDERS.soroban), null);
  assert.equal(getExplorerBaseUrl('nonsense'), 'https://stellar.expert/explorer/testnet');
});

test('buildContractExplorerUrl builds a Stellar Expert contract URL per network', () => {
  assert.equal(
    buildContractExplorerUrl('testnet', CONTRACT_ID),
    `https://stellar.expert/explorer/testnet/contract/${CONTRACT_ID}`,
  );
  assert.equal(
    buildContractExplorerUrl('mainnet', CONTRACT_ID),
    `https://stellar.expert/explorer/public/contract/${CONTRACT_ID}`,
  );
  assert.equal(
    buildContractExplorerUrl('futurenet', CONTRACT_ID),
    `https://stellar.expert/explorer/futurenet/contract/${CONTRACT_ID}`,
  );
});

test('buildContractExplorerUrl builds a Soroban explorer contract URL per network', () => {
  assert.equal(
    buildContractExplorerUrl('testnet', CONTRACT_ID, EXPLORER_PROVIDERS.soroban),
    `https://soroban-testnet.stellar.org/contract/${CONTRACT_ID}`,
  );
  assert.equal(
    buildContractExplorerUrl('mainnet', CONTRACT_ID, EXPLORER_PROVIDERS.soroban),
    `https://soroban.stellar.org/contract/${CONTRACT_ID}`,
  );
});

test('buildAccountExplorerUrl and buildTxExplorerUrl use the right path segments', () => {
  assert.equal(
    buildAccountExplorerUrl('testnet', CONTRACT_ID),
    `https://stellar.expert/explorer/testnet/account/${CONTRACT_ID}`,
  );
  assert.equal(
    buildTxExplorerUrl('testnet', TX_HASH),
    `https://stellar.expert/explorer/testnet/tx/${TX_HASH}`,
  );
});

test('buildExplorerUrl encodes identifiers so they cannot break out of the path', () => {
  assert.equal(
    buildExplorerUrl('testnet', 'contract', '../../evil'),
    'https://stellar.expert/explorer/testnet/contract/..%2F..%2Fevil',
  );
});

test('buildExplorerUrl returns null for unlinkable or unsupported inputs', () => {
  assert.equal(buildContractExplorerUrl('testnet', ''), null);
  assert.equal(buildContractExplorerUrl('testnet', '   '), null);
  assert.equal(buildContractExplorerUrl('testnet', 'CC 3J'), null);
  assert.equal(buildContractExplorerUrl('testnet', null), null);
  assert.equal(buildContractExplorerUrl('localhost', CONTRACT_ID), null);
  // The Soroban Lab explorer only hosts contracts.
  assert.equal(buildAccountExplorerUrl('testnet', CONTRACT_ID, EXPLORER_PROVIDERS.soroban), null);
  assert.equal(buildTxExplorerUrl('testnet', TX_HASH, EXPLORER_PROVIDERS.soroban), null);
  assert.equal(buildExplorerUrl('testnet', 'bogus-kind', CONTRACT_ID), null);
});

test('SAFE_EXTERNAL_LINK_PROPS opens links in a new tab safely', () => {
  assert.equal(SAFE_EXTERNAL_LINK_PROPS.target, '_blank');
  assert.equal(SAFE_EXTERNAL_LINK_PROPS.rel, 'noopener noreferrer');
  assert.ok(SAFE_EXTERNAL_LINK_PROPS.rel.split(' ').includes('noopener'));
  assert.ok(SAFE_EXTERNAL_LINK_PROPS.rel.split(' ').includes('noreferrer'));
});
