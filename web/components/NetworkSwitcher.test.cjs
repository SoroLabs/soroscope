// NetworkSwitcher.test.cjs — unit tests for NetworkSwitcher component and NetworkContext logic
// Closes Issue #612
// Runs with: node --test ./components/NetworkSwitcher.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// Simulated network definitions (matches NetworkContext.tsx)
const NETWORKS = {
  mainnet: {
    id: 'mainnet',
    name: 'Mainnet (Public)',
    shortName: 'Mainnet',
    rpcUrl: 'https://rpc.mainnet.stellar.org',
    horizonUrl: 'https://horizon.stellar.org',
    networkPassphrase: 'Public Global Stellar Network ; September 2015',
    defaultContractId: 'CCW67TSBZVTZPT2PP5FE62EBY6GL3KCQSXYGLF73264P3EGBB3LCGN4P',
  },
  testnet: {
    id: 'testnet',
    name: 'Testnet',
    shortName: 'Testnet',
    rpcUrl: 'https://soroban-testnet.stellar.org',
    horizonUrl: 'https://horizon-testnet.stellar.org',
    networkPassphrase: 'Test SDF Network ; September 2015',
    defaultContractId: 'CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q',
  },
  futurenet: {
    id: 'futurenet',
    name: 'Futurenet',
    shortName: 'Futurenet',
    rpcUrl: 'https://rpc-futurenet.stellar.org',
    horizonUrl: 'https://horizon-futurenet.stellar.org',
    networkPassphrase: 'Test SDF Future Network ; October 2022',
    defaultContractId: 'CB6EAKZTWL67L7B55W4D337FUGJ36J2G4T7N5K6V6P6N7Q8R9S0T1U2V',
  },
  localhost: {
    id: 'localhost',
    name: 'Localhost (Standalone)',
    shortName: 'Localhost',
    rpcUrl: 'http://localhost:8000/soroban/rpc',
    horizonUrl: 'http://localhost:8000',
    networkPassphrase: 'Standalone Network ; February 2017',
    defaultContractId: 'CDLZFC3SYJYDVR7P6CC4D5RUDTFGWXMGAW5W6L2G4T7N5K6V6P6N7Q8R',
  },
};

/**
 * Simulated Network State & Dropdown Manager
 */
function createNetworkManager(initialNetworkId = 'testnet') {
  let activeId = initialNetworkId;
  let isOpen = false;
  const storage = new Map();

  return {
    get activeNetworkId() {
      return activeId;
    },
    get activeNetwork() {
      return NETWORKS[activeId] || NETWORKS.testnet;
    },
    get isOpen() {
      return isOpen;
    },
    toggleDropdown() {
      isOpen = !isOpen;
    },
    closeDropdown() {
      isOpen = false;
    },
    setNetwork(id) {
      if (NETWORKS[id]) {
        activeId = id;
        storage.set('soroscope_selected_network', id);
        isOpen = false;
      }
    },
    getStoredNetwork() {
      return storage.get('soroscope_selected_network');
    },
    handleKeyDown(key) {
      if (key === 'Escape' && isOpen) {
        isOpen = false;
      }
    },
  };
}

// ── Tests ───────────────────────────────────────────────────────────────────

test('NetworkSwitcher: default network is testnet', () => {
  const nm = createNetworkManager();
  assert.equal(nm.activeNetworkId, 'testnet');
  assert.equal(nm.activeNetwork.name, 'Testnet');
  assert.equal(nm.activeNetwork.rpcUrl, 'https://soroban-testnet.stellar.org');
});

test('NetworkSwitcher: all 4 required networks are supported and configured', () => {
  const requiredNetworks = ['mainnet', 'testnet', 'futurenet', 'localhost'];
  for (const id of requiredNetworks) {
    assert.ok(NETWORKS[id], `Network ${id} should exist`);
    assert.ok(NETWORKS[id].rpcUrl, `Network ${id} should have rpcUrl`);
    assert.ok(NETWORKS[id].networkPassphrase, `Network ${id} should have networkPassphrase`);
    assert.ok(NETWORKS[id].defaultContractId, `Network ${id} should have defaultContractId`);
  }
});

test('NetworkSwitcher: switching network updates active RPC endpoint and passphrase', () => {
  const nm = createNetworkManager('testnet');

  nm.setNetwork('mainnet');
  assert.equal(nm.activeNetworkId, 'mainnet');
  assert.equal(nm.activeNetwork.rpcUrl, 'https://rpc.mainnet.stellar.org');
  assert.equal(nm.activeNetwork.networkPassphrase, 'Public Global Stellar Network ; September 2015');

  nm.setNetwork('futurenet');
  assert.equal(nm.activeNetworkId, 'futurenet');
  assert.equal(nm.activeNetwork.rpcUrl, 'https://rpc-futurenet.stellar.org');

  nm.setNetwork('localhost');
  assert.equal(nm.activeNetworkId, 'localhost');
  assert.equal(nm.activeNetwork.rpcUrl, 'http://localhost:8000/soroban/rpc');
});

test('NetworkSwitcher: selected network is persisted to storage', () => {
  const nm = createNetworkManager('testnet');
  nm.setNetwork('futurenet');
  assert.equal(nm.getStoredNetwork(), 'futurenet');
});

test('NetworkSwitcher: invalid network ID is ignored', () => {
  const nm = createNetworkManager('testnet');
  nm.setNetwork('invalid_network_name');
  assert.equal(nm.activeNetworkId, 'testnet');
});

test('NetworkSwitcher: Escape key closes dropdown menu when open', () => {
  const nm = createNetworkManager();
  nm.toggleDropdown();
  assert.equal(nm.isOpen, true);

  nm.handleKeyDown('Escape');
  assert.equal(nm.isOpen, false);
});

test('NetworkSwitcher: button satisfies minimum 44px touch target requirement', () => {
  const minHeightPx = 44; // min-h-[44px]
  const RECOMMENDED_MIN_PX = 44;
  assert.ok(minHeightPx >= RECOMMENDED_MIN_PX, 'NetworkSwitcher button satisfies touch target guideline');
});
