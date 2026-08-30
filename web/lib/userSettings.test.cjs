// userSettings.test.cjs — unit tests for custom RPC/indexer preference storage
// Closes Issue #622
// Runs with: node --test ./lib/userSettings.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  SETTINGS_STORAGE_KEY,
  DEFAULT_SETTINGS,
  validateEndpointUrl,
  validateSettings,
  normalizeSettings,
  loadSettings,
  saveSettings,
  clearSettings,
  resolveEndpoint,
} = require('./userSettings');

/** Minimal in-memory LocalStorage stand-in. */
function createStorage(initial = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (key) => (map.has(key) ? map.get(key) : null),
    setItem: (key, value) => map.set(key, String(value)),
    removeItem: (key) => map.delete(key),
    get size() {
      return map.size;
    },
  };
}

// ── URL validation ──────────────────────────────────────────────────────────

test('validateEndpointUrl: accepts http and https URLs', () => {
  assert.equal(validateEndpointUrl('https://rpc.example.com').valid, true);
  assert.equal(validateEndpointUrl('http://localhost:8000/soroban/rpc').valid, true);
});

test('validateEndpointUrl: strips trailing slashes when normalizing', () => {
  assert.equal(validateEndpointUrl('https://rpc.example.com///').normalized, 'https://rpc.example.com');
});

test('validateEndpointUrl: treats blank input as "use the default"', () => {
  assert.deepEqual(validateEndpointUrl(''), { valid: true, normalized: '', error: null });
  assert.deepEqual(validateEndpointUrl('   '), { valid: true, normalized: '', error: null });
  assert.equal(validateEndpointUrl(undefined).valid, true);
});

test('validateEndpointUrl: rejects blank input when allowEmpty is false', () => {
  const result = validateEndpointUrl('', { allowEmpty: false });
  assert.equal(result.valid, false);
  assert.match(result.error, /required/i);
});

test('validateEndpointUrl: rejects a URL with no protocol', () => {
  const result = validateEndpointUrl('rpc.example.com');
  assert.equal(result.valid, false);
  assert.match(result.error, /including the protocol/i);
});

test('validateEndpointUrl: rejects non-http protocols', () => {
  const result = validateEndpointUrl('ftp://rpc.example.com');
  assert.equal(result.valid, false);
  assert.match(result.error, /Unsupported protocol/);
});

// ── Normalization ───────────────────────────────────────────────────────────

test('normalizeSettings: fills in defaults for junk input', () => {
  assert.deepEqual(normalizeSettings(null), DEFAULT_SETTINGS);
  assert.deepEqual(normalizeSettings('nope'), DEFAULT_SETTINGS);
  assert.deepEqual(normalizeSettings({}), DEFAULT_SETTINGS);
});

test('normalizeSettings: drops invalid endpoints rather than storing them', () => {
  const normalized = normalizeSettings({ rpcUrl: 'not a url', indexerUrl: 'https://api.example.com' });
  assert.equal(normalized.rpcUrl, '');
  assert.equal(normalized.indexerUrl, 'https://api.example.com');
});

test('normalizeSettings: clamps the request timeout into range', () => {
  assert.equal(normalizeSettings({ requestTimeoutMs: 5 }).requestTimeoutMs, 1000);
  assert.equal(normalizeSettings({ requestTimeoutMs: 999999 }).requestTimeoutMs, 120000);
  assert.equal(normalizeSettings({ requestTimeoutMs: '4200' }).requestTimeoutMs, 4200);
  assert.equal(normalizeSettings({ requestTimeoutMs: NaN }).requestTimeoutMs, 15000);
});

// ── Form validation ─────────────────────────────────────────────────────────

test('validateSettings: reports per-field errors', () => {
  const result = validateSettings({ rpcUrl: 'ws://bad', indexerUrl: 'https://ok.example.com' });
  assert.equal(result.valid, false);
  assert.ok(result.errors.rpcUrl);
  assert.equal(result.errors.indexerUrl, undefined);
});

test('validateSettings: passes when both endpoints are blank', () => {
  assert.equal(validateSettings({ rpcUrl: '', indexerUrl: '' }).valid, true);
});

// ── Persistence ─────────────────────────────────────────────────────────────

test('loadSettings: returns defaults when nothing is stored', () => {
  assert.deepEqual(loadSettings(createStorage()), DEFAULT_SETTINGS);
});

test('loadSettings: returns defaults when storage is unavailable', () => {
  assert.deepEqual(loadSettings(null), DEFAULT_SETTINGS);
});

test('loadSettings: recovers from corrupt JSON instead of throwing', () => {
  const storage = createStorage({ [SETTINGS_STORAGE_KEY]: '{ not json' });
  assert.deepEqual(loadSettings(storage), DEFAULT_SETTINGS);
});

test('saveSettings then loadSettings round-trips normalized values', () => {
  const storage = createStorage();
  const written = saveSettings(
    { rpcUrl: 'https://my-rpc.example.com/', indexerUrl: '', requestTimeoutMs: 3000 },
    storage,
  );

  assert.equal(written.rpcUrl, 'https://my-rpc.example.com');
  assert.deepEqual(loadSettings(storage), written);
});

test('saveSettings: writes under the documented storage key', () => {
  const storage = createStorage();
  saveSettings({ indexerUrl: 'https://api.example.com' }, storage);
  const raw = JSON.parse(storage.getItem(SETTINGS_STORAGE_KEY));
  assert.equal(raw.indexerUrl, 'https://api.example.com');
});

test('clearSettings: removes the stored value and returns defaults', () => {
  const storage = createStorage();
  saveSettings({ rpcUrl: 'https://my-rpc.example.com' }, storage);
  assert.equal(storage.size, 1);

  assert.deepEqual(clearSettings(storage), DEFAULT_SETTINGS);
  assert.equal(storage.size, 0);
  assert.deepEqual(loadSettings(storage), DEFAULT_SETTINGS);
});

// ── Endpoint resolution ─────────────────────────────────────────────────────

test('resolveEndpoint: prefers a valid custom URL over the fallback', () => {
  assert.equal(
    resolveEndpoint('https://my-rpc.example.com', 'https://soroban-testnet.stellar.org'),
    'https://my-rpc.example.com',
  );
});

test('resolveEndpoint: falls back when the custom URL is blank or invalid', () => {
  assert.equal(resolveEndpoint('', 'http://localhost:8080'), 'http://localhost:8080');
  assert.equal(resolveEndpoint('nonsense', 'http://localhost:8080'), 'http://localhost:8080');
});

test('resolveEndpoint: strips trailing slashes from the fallback too', () => {
  assert.equal(resolveEndpoint('', 'http://localhost:8080/'), 'http://localhost:8080');
});
