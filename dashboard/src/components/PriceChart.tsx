"use client";
import { useEffect, useState } from "react";

type Range = "1H" | "24H" | "7D" | "30D";
type Point = { t: number; price: number };

const RANGES: Range[] = ["1H", "24H", "7D", "30D"];

export default function PriceChart({ asset = "XLM" }: { asset?: string }) {
  const [range, setRange] = useState<Range>("24H");
  const [data, setData] = useState<Point[] | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setData(null);
    fetch(`/sep40?asset=${encodeURIComponent(asset)}&range=${range}`)
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error(String(r.status)))))
      .then((j) => {
        if (cancelled) return;
        setData(Array.isArray(j) ? j : j.history ?? []);
      })
      .catch(() => {
        if (!cancelled) setData([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [asset, range]);

  return (
    <div data-testid="price-chart">
      <div role="tablist" aria-label="Time range">
        {RANGES.map((r) => (
          <button
            key={r}
            role="tab"
            aria-selected={range === r}
            onClick={() => setRange(r)}
          >
            {r}
          </button>
        ))}
      </div>
      {loading ? (
        <div data-testid="price-chart-skeleton" aria-busy="true">
          Loading price history…
        </div>
      ) : !data || data.length === 0 ? (
        <div data-testid="price-chart-empty">No price history available.</div>
      ) : (
        <ul data-testid="price-chart-data">
          {data.map((p) => (
            <li key={p.t}>
              {new Date(p.t).toISOString()}: {p.price}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
