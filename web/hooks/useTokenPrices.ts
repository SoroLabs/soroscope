"use client";

import { useEffect, useRef, useState } from "react";
import { fetchPrices, PriceMap } from "../lib/priceFeed";

const DEFAULT_POLL_MS = 30_000;

export interface UseTokenPricesResult {
  prices: PriceMap;
  loading: boolean;
  error: string | null;
  /** Epoch millis of the most recent successful refresh, or null. */
  lastUpdated: number | null;
}

/**
 * Subscribes to real-time USD prices for the given asset `symbols`, polling the
 * price feed on an interval so quotes stay live while the view is open.
 *
 * The symbol list is joined into a stable dependency key so passing a freshly
 * constructed array each render does not restart the poll loop.
 */
export function useTokenPrices(
  symbols: string[],
  pollMs: number = DEFAULT_POLL_MS,
): UseTokenPricesResult {
  const [prices, setPrices] = useState<PriceMap>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<number | null>(null);

  const key = Array.from(new Set(symbols)).sort().join(",");
  const activeRef = useRef<AbortController | null>(null);

  useEffect(() => {
    if (!key) {
      setPrices({});
      setError(null);
      return;
    }

    const list = key.split(",");
    let cancelled = false;
    let timer: ReturnType<typeof setInterval> | null = null;

    const load = async () => {
      activeRef.current?.abort();
      const controller = new AbortController();
      activeRef.current = controller;
      setLoading(true);
      try {
        const next = await fetchPrices(list, controller.signal);
        if (cancelled || controller.signal.aborted) return;
        setPrices(next);
        setError(null);
        setLastUpdated(Date.now());
      } catch (err: unknown) {
        if (cancelled || controller.signal.aborted) return;
        setError(err instanceof Error ? err.message : "Failed to load prices");
      } finally {
        if (!cancelled && !controller.signal.aborted) setLoading(false);
      }
    };

    load();
    if (pollMs > 0) timer = setInterval(load, pollMs);

    return () => {
      cancelled = true;
      activeRef.current?.abort();
      if (timer) clearInterval(timer);
    };
  }, [key, pollMs]);

  return { prices, loading, error, lastUpdated };
}
