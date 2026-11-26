/**
 * User preference storage for custom Soroban RPC and indexer endpoints.
 *
 * Power users running a self-hosted RPC node or indexer can point SoroScope at
 * their own infrastructure. Preferences live in LocalStorage only — nothing is
 * sent to the backend — and every read is defensive because the value can be
 * edited by hand or left over from an older schema.
 */

const SETTINGS_STORAGE_KEY = 'soroscope-user-settings';

/** Empty string means "fall back to the built-in endpoint for the network". */
const DEFAULT_SETTINGS = {
  rpcUrl: '',
  indexerUrl: '',
  requestTimeoutMs: 15000,
};

const MIN_TIMEOUT_MS = 1000;
const MAX_TIMEOUT_MS = 120000;

/**
 * Validate a user supplied endpoint.
 *
 * @param {string} value
 * @param {{ allowEmpty?: boolean }} [options]
 * @returns {{ valid: boolean, normalized: string, error: string|null }}
 */
function validateEndpointUrl(value, options = {}) {
  const allowEmpty = options.allowEmpty !== false;
  const raw = typeof value === 'string' ? value.trim() : '';

  if (raw === '') {
    return allowEmpty
      ? { valid: true, normalized: '', error: null }
      : { valid: false, normalized: '', error: 'Endpoint URL is required' };
  }

  let parsed;
  try {
    parsed = new URL(raw);
  } catch {
    return {
      valid: false,
      normalized: raw,
      error: 'Enter a full URL including the protocol, e.g. https://rpc.example.com',
    };
  }

  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    return {
      valid: false,
      normalized: raw,
      error: `Unsupported protocol "${parsed.protocol}" — use http:// or https://`,
    };
  }

  if (!parsed.hostname) {
    return { valid: false, normalized: raw, error: 'URL is missing a hostname' };
  }

  // Strip trailing slashes so path joining downstream never doubles up.
  const normalized = parsed.toString().replace(/\/+$/, '');
  return { valid: true, normalized, error: null };
}

function clampTimeout(value) {
  const num = typeof value === 'number' ? value : Number(value);
  if (!Number.isFinite(num)) return DEFAULT_SETTINGS.requestTimeoutMs;
  return Math.min(MAX_TIMEOUT_MS, Math.max(MIN_TIMEOUT_MS, Math.round(num)));
}

/** Coerce an arbitrary object into a complete, valid settings object. */
function normalizeSettings(raw) {
  const input = raw && typeof raw === 'object' ? raw : {};
  const rpc = validateEndpointUrl(input.rpcUrl);
  const indexer = validateEndpointUrl(input.indexerUrl);

  return {
    rpcUrl: rpc.valid ? rpc.normalized : '',
    indexerUrl: indexer.valid ? indexer.normalized : '',
    requestTimeoutMs: clampTimeout(input.requestTimeoutMs),
  };
}

/** Validate a whole form payload before saving. */
function validateSettings(raw) {
  const input = raw && typeof raw === 'object' ? raw : {};
  const errors = {};

  const rpc = validateEndpointUrl(input.rpcUrl);
  if (!rpc.valid) errors.rpcUrl = rpc.error;

  const indexer = validateEndpointUrl(input.indexerUrl);
  if (!indexer.valid) errors.indexerUrl = indexer.error;

  return { valid: Object.keys(errors).length === 0, errors };
}

function resolveStorage(storage) {
  if (storage) return storage;
  if (typeof window !== 'undefined' && window.localStorage) return window.localStorage;
  return null;
}

/**
 * Read saved settings. Always returns a complete object, falling back to
 * defaults when storage is unavailable or the stored value is corrupt.
 */
function loadSettings(storage) {
  const store = resolveStorage(storage);
  if (!store) return { ...DEFAULT_SETTINGS };

  try {
    const raw = store.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) return { ...DEFAULT_SETTINGS };
    return normalizeSettings(JSON.parse(raw));
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

/** Persist settings, returning the normalized object that was written. */
function saveSettings(settings, storage) {
  const normalized = normalizeSettings(settings);
  const store = resolveStorage(storage);
  if (!store) return normalized;

  try {
    store.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(normalized));
  } catch (error) {
    console.warn('Failed to persist SoroScope settings:', error);
  }

  return normalized;
}

function clearSettings(storage) {
  const store = resolveStorage(storage);
  if (store) {
    try {
      store.removeItem(SETTINGS_STORAGE_KEY);
    } catch (error) {
      console.warn('Failed to clear SoroScope settings:', error);
    }
  }
  return { ...DEFAULT_SETTINGS };
}

/** Custom endpoint wins over the built-in/network default when set. */
function resolveEndpoint(customUrl, fallbackUrl) {
  const custom = validateEndpointUrl(customUrl);
  if (custom.valid && custom.normalized !== '') return custom.normalized;
  return typeof fallbackUrl === 'string' ? fallbackUrl.replace(/\/+$/, '') : '';
}

module.exports = {
  SETTINGS_STORAGE_KEY,
  DEFAULT_SETTINGS,
  MIN_TIMEOUT_MS,
  MAX_TIMEOUT_MS,
  validateEndpointUrl,
  validateSettings,
  normalizeSettings,
  loadSettings,
  saveSettings,
  clearSettings,
  resolveEndpoint,
};
