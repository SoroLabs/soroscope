/**
 * TelemetryContext
 *
 * Provides app-wide access to:
 *  - `telemetryEnabled`  — current opt-in state (false if DNT is set)
 *  - `dntActive`         — whether the browser has asserted DNT / GPC
 *  - `setEnabled(bool)`  — toggle opt-in preference from the settings UI
 *  - `track(event, props)` — fire a named event (no-op if disabled / DNT)
 *
 * Wrap the app root with <TelemetryProvider> in _app.tsx.
 */

import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import {
  hasDntSignal,
  isTelemetryEnabled,
  setTelemetryEnabled,
  track as coreTrack,
} from '../lib/telemetry';

// ─── Context shape ────────────────────────────────────────────────────────────

interface TelemetryContextValue {
  /** True only when the user opted in AND the browser has not asserted DNT. */
  telemetryEnabled: boolean;
  /** True when the browser has asserted DNT or Global Privacy Control. */
  dntActive: boolean;
  /** Persist the user's opt-in or opt-out choice. */
  setEnabled: (enabled: boolean) => void;
  /** Track a named event.  No-op when telemetry is disabled or DNT is set. */
  track: (event: string, properties?: Record<string, unknown>) => void;
}

const TelemetryContext = createContext<TelemetryContextValue>({
  telemetryEnabled: false,
  dntActive: false,
  setEnabled: () => undefined,
  track: () => undefined,
});

// ─── Provider ─────────────────────────────────────────────────────────────────

export function TelemetryProvider({ children }: { children: React.ReactNode }): JSX.Element {
  // Read the initial state after mount so SSR renders are consistent (both
  // always start as disabled — localStorage is not available on the server).
  const [telemetryEnabled, setTelemetryEnabled_] = useState<boolean>(false);
  const [dntActive, setDntActive] = useState<boolean>(false);

  useEffect(() => {
    const dnt = hasDntSignal();
    setDntActive(dnt);
    setTelemetryEnabled_(dnt ? false : isTelemetryEnabled());
  }, []);

  const setEnabled = useCallback((enabled: boolean) => {
    // DNT takes precedence; the stored preference can still be written so the
    // UI reflects the user's intent, but isTelemetryEnabled() will return false.
    setTelemetryEnabled(enabled);
    setTelemetryEnabled_(hasDntSignal() ? false : enabled);
  }, []);

  const track = useCallback(
    (event: string, properties: Record<string, unknown> = {}) => {
      coreTrack(event, properties);
    },
    [],
  );

  const value = useMemo<TelemetryContextValue>(
    () => ({ telemetryEnabled, dntActive, setEnabled, track }),
    [telemetryEnabled, dntActive, setEnabled, track],
  );

  return (
    <TelemetryContext.Provider value={value}>{children}</TelemetryContext.Provider>
  );
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

/**
 * Access telemetry state and helpers from any component.
 *
 * @example
 * const { track } = useTelemetry();
 * track('simulate_contract', { network });
 */
export function useTelemetry(): TelemetryContextValue {
  return useContext(TelemetryContext);
}

export default TelemetryContext;
