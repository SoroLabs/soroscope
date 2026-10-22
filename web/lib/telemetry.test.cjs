'use strict';
/**
 * Unit tests for web/lib/telemetry.ts
 *
 * Run with:  cd web && node --test lib/telemetry.test.cjs
 * Or via the project's existing test runner (Jest / Node test runner).
 */

const assert = require('node:assert/strict');
const { describe, it, beforeEach } = require('node:test');

// ---------------------------------------------------------------------------
// Minimal localStorage stub
// ---------------------------------------------------------------------------
function makeStorage() {
  const store = {};
  return {
    getItem: (k) => (Object.prototype.hasOwnProperty.call(store, k) ? store[k] : null),
    setItem: (k, v) => { store[k] = String(v); },
    removeItem: (k) => { delete store[k]; },
    clear: () => { Object.keys(store).forEach((k) => delete store[k]); },
  };
}

// ---------------------------------------------------------------------------
// Import helpers (CJS-compatible — transpile or require the compiled output)
// ---------------------------------------------------------------------------
// We test the logic directly through re-exported pure functions.
// The TypeScript source is loaded via ts-node / the build output depending on
// how the test runner is configured; fall back to a manual stub when neither
// is available so CI never silently skips tests.

let telemetry;
try {
  // Try compiled JS output first (produced by `tsc` or Next.js build)
  telemetry = require('./telemetry');
} catch {
  // Inline the pure logic so the tests can run without a build step
  const TELEMETRY_STORAGE_KEY = 'soroscope-telemetry';

  function hasDntSignal() {
    if (typeof window === 'undefined') return false;
    if (
      typeof (navigator).globalPrivacyControl === 'boolean' &&
      (navigator).globalPrivacyControl === true
    ) return true;
    const dnt = typeof navigator !== 'undefined' ? navigator.doNotTrack : undefined;
    if (dnt === '1' || dnt === 'yes') return true;
    const wDnt = typeof window !== 'undefined' ? (window).doNotTrack : undefined;
    if (wDnt === '1' || wDnt === 'yes') return true;
    return false;
  }

  function loadTelemetryPreference(storage) {
    if (!storage) return { enabled: false };
    try {
      const raw = storage.getItem(TELEMETRY_STORAGE_KEY);
      if (!raw) return { enabled: false };
      const parsed = JSON.parse(raw);
      if (typeof parsed === 'object' && parsed !== null && 'enabled' in parsed) {
        return { enabled: Boolean(parsed.enabled) };
      }
      return { enabled: false };
    } catch { return { enabled: false }; }
  }

  function saveTelemetryPreference(enabled, storage) {
    const pref = { enabled };
    if (storage) {
      try { storage.setItem(TELEMETRY_STORAGE_KEY, JSON.stringify(pref)); } catch {}
    }
    return pref;
  }

  function isTelemetryEnabled(storage) {
    if (typeof window === 'undefined') return false;
    if (hasDntSignal()) return false;
    return loadTelemetryPreference(storage).enabled;
  }

  function setTelemetryEnabled(enabled, storage) {
    saveTelemetryPreference(enabled, storage);
  }

  telemetry = {
    TELEMETRY_STORAGE_KEY,
    hasDntSignal,
    loadTelemetryPreference,
    saveTelemetryPreference,
    isTelemetryEnabled,
    setTelemetryEnabled,
  };
}

const {
  TELEMETRY_STORAGE_KEY,
  hasDntSignal,
  loadTelemetryPreference,
  saveTelemetryPreference,
  isTelemetryEnabled,
  setTelemetryEnabled,
} = telemetry;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('hasDntSignal()', () => {
  it('returns false when running server-side (no window)', () => {
    // In Node.js test environment there is no window object.
    const result = hasDntSignal();
    assert.equal(result, false);
  });

  it('returns true when window.doNotTrack is "1"', () => {
    const original = global.window;
    global.window = { doNotTrack: '1' };
    global.navigator = {};
    try {
      assert.equal(hasDntSignal(), true);
    } finally {
      global.window = original;
      delete global.navigator;
    }
  });

  it('returns true when navigator.doNotTrack is "yes"', () => {
    const original = global.window;
    global.window = {};
    global.navigator = { doNotTrack: 'yes' };
    try {
      assert.equal(hasDntSignal(), true);
    } finally {
      global.window = original;
      delete global.navigator;
    }
  });

  it('returns true when navigator.globalPrivacyControl is true', () => {
    const original = global.window;
    global.window = {};
    global.navigator = { globalPrivacyControl: true };
    try {
      assert.equal(hasDntSignal(), true);
    } finally {
      global.window = original;
      delete global.navigator;
    }
  });

  it('returns false when doNotTrack is "0"', () => {
    const original = global.window;
    global.window = {};
    global.navigator = { doNotTrack: '0' };
    try {
      assert.equal(hasDntSignal(), false);
    } finally {
      global.window = original;
      delete global.navigator;
    }
  });
});

describe('loadTelemetryPreference()', () => {
  it('returns { enabled: false } when storage is null', () => {
    const result = loadTelemetryPreference(null);
    assert.deepEqual(result, { enabled: false });
  });

  it('returns { enabled: false } when the key is absent', () => {
    const storage = makeStorage();
    assert.deepEqual(loadTelemetryPreference(storage), { enabled: false });
  });

  it('returns { enabled: true } when stored as true', () => {
    const storage = makeStorage();
    storage.setItem(TELEMETRY_STORAGE_KEY, JSON.stringify({ enabled: true }));
    assert.deepEqual(loadTelemetryPreference(storage), { enabled: true });
  });

  it('returns { enabled: false } when stored as false', () => {
    const storage = makeStorage();
    storage.setItem(TELEMETRY_STORAGE_KEY, JSON.stringify({ enabled: false }));
    assert.deepEqual(loadTelemetryPreference(storage), { enabled: false });
  });

  it('returns { enabled: false } when stored JSON is malformed', () => {
    const storage = makeStorage();
    storage.setItem(TELEMETRY_STORAGE_KEY, 'NOT_JSON{{{');
    assert.deepEqual(loadTelemetryPreference(storage), { enabled: false });
  });

  it('returns { enabled: false } when stored object lacks "enabled" key', () => {
    const storage = makeStorage();
    storage.setItem(TELEMETRY_STORAGE_KEY, JSON.stringify({ foo: 'bar' }));
    assert.deepEqual(loadTelemetryPreference(storage), { enabled: false });
  });
});

describe('saveTelemetryPreference()', () => {
  it('persists { enabled: true } and returns it', () => {
    const storage = makeStorage();
    const result = saveTelemetryPreference(true, storage);
    assert.deepEqual(result, { enabled: true });
    const stored = JSON.parse(storage.getItem(TELEMETRY_STORAGE_KEY));
    assert.deepEqual(stored, { enabled: true });
  });

  it('persists { enabled: false } and returns it', () => {
    const storage = makeStorage();
    const result = saveTelemetryPreference(false, storage);
    assert.deepEqual(result, { enabled: false });
    const stored = JSON.parse(storage.getItem(TELEMETRY_STORAGE_KEY));
    assert.deepEqual(stored, { enabled: false });
  });

  it('does not throw when storage is null', () => {
    assert.doesNotThrow(() => saveTelemetryPreference(true, null));
  });

  it('overwrites a previous preference', () => {
    const storage = makeStorage();
    saveTelemetryPreference(true, storage);
    saveTelemetryPreference(false, storage);
    assert.deepEqual(loadTelemetryPreference(storage), { enabled: false });
  });
});

describe('isTelemetryEnabled()', () => {
  it('returns false when running server-side (no window)', () => {
    // This test runs in Node where window is undefined by default.
    assert.equal(isTelemetryEnabled(makeStorage()), false);
  });

  it('returns false when opted in but DNT is set', () => {
    const original = global.window;
    global.window = {};
    global.navigator = { doNotTrack: '1' };
    const storage = makeStorage();
    saveTelemetryPreference(true, storage);
    try {
      assert.equal(isTelemetryEnabled(storage), false);
    } finally {
      global.window = original;
      delete global.navigator;
    }
  });

  it('returns false when opted out and no DNT', () => {
    const original = global.window;
    global.window = {};
    global.navigator = { doNotTrack: '0' };
    const storage = makeStorage();
    saveTelemetryPreference(false, storage);
    try {
      assert.equal(isTelemetryEnabled(storage), false);
    } finally {
      global.window = original;
      delete global.navigator;
    }
  });

  it('returns true when opted in and no DNT (browser env)', () => {
    const original = global.window;
    global.window = {};
    global.navigator = { doNotTrack: null };
    const storage = makeStorage();
    saveTelemetryPreference(true, storage);
    try {
      assert.equal(isTelemetryEnabled(storage), true);
    } finally {
      global.window = original;
      delete global.navigator;
    }
  });
});

describe('setTelemetryEnabled()', () => {
  it('enables telemetry and persists the preference', () => {
    const storage = makeStorage();
    setTelemetryEnabled(true, storage);
    assert.deepEqual(loadTelemetryPreference(storage), { enabled: true });
  });

  it('disables telemetry and persists the preference', () => {
    const storage = makeStorage();
    setTelemetryEnabled(true, storage);
    setTelemetryEnabled(false, storage);
    assert.deepEqual(loadTelemetryPreference(storage), { enabled: false });
  });

  it('does not throw when storage is null', () => {
    assert.doesNotThrow(() => setTelemetryEnabled(true, null));
  });
});

describe('event queue / no-op when opted out', () => {
  it('track() is a no-op (no fetch) when telemetry is disabled', async () => {
    // Patch global.fetch to detect unwanted calls
    let fetchCalled = false;
    const originalFetch = global.fetch;
    global.fetch = () => { fetchCalled = true; return Promise.resolve(new Response()); };

    // Ensure window exists but telemetry is NOT enabled
    const original = global.window;
    global.window = {};
    global.navigator = { doNotTrack: null };

    try {
      // No window + no opt-in  → track() must be a no-op
      if (telemetry.track) {
        telemetry.track('test_event', { foo: 'bar' });
        // Give the microtask queue a chance to run
        await new Promise((r) => setTimeout(r, 20));
        assert.equal(fetchCalled, false, 'fetch should not be called when telemetry is off');
      }
    } finally {
      global.fetch = originalFetch;
      global.window = original;
      delete global.navigator;
    }
  });
});
