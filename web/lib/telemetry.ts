/**
 * Anonymous Telemetry Module — SoroScope Web Dashboard
 *
 * Design principles:
 *  - Strictly opt-in. No data is ever collected before the user explicitly
 *    enables telemetry.
 *  - Do Not Track (DNT) / Global Privacy Control (GPC) respected automatically.
 *    If the browser asserts either signal the module stays permanently off,
 *    regardless of the stored preference.
 *  - Anonymity by construction. No IP, no user identifier, no session cookie.
 *    Events carry only a per-session ephemeral ID (reset on every page load),
 *    an app version string, and a monotonic event timestamp.
 *  - Fire-and-forget delivery.  Failed sends are silently discarded — telemetry
 *    is never allowed to degrade the user experience.
 *  - All data remains on-device when delivery is disabled or blocked.
 *
 * Storage key: "soroscope-telemetry" in localStorage.
 * Opt-in value: { enabled: boolean }
 *
 * Usage:
 *   import { track, isTelemetryEnabled, setTelemetryEnabled } from './telemetry';
 *
 *   // Check current state
 *   const on = isTelemetryEnabled();
 *
 *   // Toggle from settings UI
 *   setTelemetryEnabled(true);
 *
 *   // Instrument a user action
 *   track('simulate_contract', { network: 'testnet' });
 */

// ─── Constants ───────────────────────────────────────────────────────────────

export const TELEMETRY_STORAGE_KEY = 'soroscope-telemetry';

/**
 * Replace with a real ingestion URL when one is set up.
 * The module gracefully no-ops if the value is an empty string.
 */
const TELEMETRY_ENDPOINT =
  (typeof process !== 'undefined' && process.env.NEXT_PUBLIC_TELEMETRY_URL) || '';

// ─── Types ───────────────────────────────────────────────────────────────────

export interface TelemetryPreference {
  enabled: boolean;
}

export interface TelemetryEvent {
  /** Short snake_case name identifying the action, e.g. "simulate_contract". */
  event: string;
  /** Arbitrary structured payload.  Must contain no PII. */
  properties: Record<string, unknown>;
  /** ISO-8601 timestamp (client wall-clock). */
  timestamp: string;
  /** Ephemeral random ID, regenerated on every page load — never persisted. */
  sessionId: string;
  /** app version from package.json injected at build time, or "dev". */
  appVersion: string;
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/** True only in a real browser environment (not SSR). */
function isBrowser(): boolean {
  return typeof window !== 'undefined';
}

/**
 * Detect Do Not Track (DNT) and Global Privacy Control (GPC) browser signals.
 *
 * Returns true when either signal requests that tracking be disabled.
 * Returns false when neither signal is present, or when running on the server.
 */
export function hasDntSignal(): boolean {
  if (!isBrowser()) return false;

  // Global Privacy Control — W3C draft: navigator.globalPrivacyControl
  // https://globalprivacycontrol.org/
  if (
    typeof (navigator as unknown as Record<string, unknown>).globalPrivacyControl === 'boolean' &&
    (navigator as unknown as Record<string, boolean>).globalPrivacyControl === true
  ) {
    return true;
  }

  // Do Not Track — legacy, but still widely supported
  const dnt = navigator.doNotTrack;
  if (dnt === '1' || dnt === 'yes') return true;

  // IE / old Edge stored it on window instead
  const windowDnt =
    typeof (window as unknown as Record<string, unknown>).doNotTrack === 'string'
      ? (window as unknown as Record<string, string>).doNotTrack
      : undefined;
  if (windowDnt === '1' || windowDnt === 'yes') return true;

  return false;
}

// ─── Opt-in storage ──────────────────────────────────────────────────────────

function resolveStorage(): Storage | null {
  if (typeof window !== 'undefined' && window.localStorage) return window.localStorage;
  return null;
}

/**
 * Read the stored telemetry preference.
 * Returns { enabled: false } when storage is unavailable, the key is absent,
 * or the stored JSON is malformed.
 */
export function loadTelemetryPreference(storage?: Storage): TelemetryPreference {
  const store = storage ?? resolveStorage();
  if (!store) return { enabled: false };

  try {
    const raw = store.getItem(TELEMETRY_STORAGE_KEY);
    if (!raw) return { enabled: false };
    const parsed = JSON.parse(raw) as unknown;
    if (typeof parsed === 'object' && parsed !== null && 'enabled' in parsed) {
      return { enabled: Boolean((parsed as { enabled: unknown }).enabled) };
    }
    return { enabled: false };
  } catch {
    return { enabled: false };
  }
}

/**
 * Persist the user's telemetry preference.
 * If DNT/GPC is asserted, the preference is stored but telemetry will still
 * be blocked at runtime — respecting the browser signal takes precedence.
 */
export function saveTelemetryPreference(
  enabled: boolean,
  storage?: Storage,
): TelemetryPreference {
  const store = storage ?? resolveStorage();
  const pref: TelemetryPreference = { enabled };
  if (store) {
    try {
      store.setItem(TELEMETRY_STORAGE_KEY, JSON.stringify(pref));
    } catch {
      // Ignore write failures (quota exceeded, private browsing restrictions).
    }
  }
  return pref;
}

// ─── Session ID ──────────────────────────────────────────────────────────────

/**
 * Ephemeral session ID, generated once per module load (i.e. per page load).
 * It is never written to localStorage — by design it rotates with every visit
 * so it cannot be used to correlate sessions across page loads.
 */
const SESSION_ID: string = (() => {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  // Polyfill for older browsers / Jest environment
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
})();

const APP_VERSION: string =
  (typeof process !== 'undefined' && process.env.NEXT_PUBLIC_APP_VERSION) || 'dev';

// ─── Event queue ─────────────────────────────────────────────────────────────

/**
 * Lightweight in-memory queue.  Events are held here while the module is
 * booting or while an inflight send is in progress.  If the user opts out mid-
 * session the queue is drained without sending.
 */
const eventQueue: TelemetryEvent[] = [];

let isFlushing = false;

// ─── Public API ──────────────────────────────────────────────────────────────

/**
 * Check whether telemetry is currently active.
 *
 * Returns true only when ALL of the following hold:
 *  1. We're in a browser (not SSR).
 *  2. The user has explicitly opted in.
 *  3. The browser has not asserted DNT or GPC.
 */
export function isTelemetryEnabled(storage?: Storage): boolean {
  if (!isBrowser()) return false;
  if (hasDntSignal()) return false;
  return loadTelemetryPreference(storage).enabled;
}

/**
 * Toggle telemetry on or off, persisting the choice to LocalStorage.
 *
 * Note: even when set to true, calls to track() are silently swallowed if
 * hasDntSignal() returns true — the browser-level signal always wins.
 */
export function setTelemetryEnabled(enabled: boolean, storage?: Storage): void {
  saveTelemetryPreference(enabled, storage);
  if (!enabled) {
    // Drain the queue without sending — user has opted out.
    eventQueue.length = 0;
  }
}

/**
 * Record a telemetry event.
 *
 * - Is a strict no-op when telemetry is disabled or DNT is set.
 * - Enqueues the event and schedules a non-blocking flush.
 * - Does NOT throw under any circumstances.
 *
 * @param event    Short descriptive name, e.g. "simulate_contract".
 * @param properties  Plain JSON-serialisable object — NO PII.
 */
export function track(event: string, properties: Record<string, unknown> = {}): void {
  try {
    if (!isTelemetryEnabled()) return;

    const entry: TelemetryEvent = {
      event,
      properties,
      timestamp: new Date().toISOString(),
      sessionId: SESSION_ID,
      appVersion: APP_VERSION,
    };

    eventQueue.push(entry);
    scheduleFlush();
  } catch {
    // Swallow all errors — telemetry must never break the app.
  }
}

// ─── Delivery ────────────────────────────────────────────────────────────────

function scheduleFlush(): void {
  if (isFlushing) return;
  // Use a microtask-deferred send so track() returns synchronously to callers.
  queueMicrotask(flush);
}

/**
 * Drain the in-memory event queue and deliver all events to the ingestion
 * endpoint in a single POST request.
 *
 * All errors are silently discarded — telemetry delivery is best-effort only.
 */
async function flush(): Promise<void> {
  if (isFlushing) return;
  if (!TELEMETRY_ENDPOINT) {
    // No endpoint configured — discard the queue silently.
    eventQueue.length = 0;
    return;
  }

  const batch = eventQueue.splice(0);
  if (batch.length === 0) return;

  isFlushing = true;
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 5_000);

    await fetch(TELEMETRY_ENDPOINT, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      // keepalive lets the request survive a page unload (best-effort)
      keepalive: true,
      body: JSON.stringify({ events: batch }),
      signal: controller.signal,
    });

    clearTimeout(timer);
  } catch {
    // Fire-and-forget: failed sends are silently discarded.
  } finally {
    isFlushing = false;
  }
}

// ─── Convenience re-exports ──────────────────────────────────────────────────

export { SESSION_ID };
