"use client";
import { useEffect, useState } from "react";

type Flag = { key: string; enabled: boolean };

export default function AdminControls() {
  const [flags, setFlags] = useState<Flag[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    fetch("/api/config")
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error())))
      .then((j) => {
        if (!cancelled) setFlags(Array.isArray(j) ? j : j.flags ?? []);
      })
      .catch(() => {
        if (!cancelled) setFlags([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const toggle = async (key: string) => {
    const prev = flags;
    const next = flags.map((f) =>
      f.key === key ? { ...f, enabled: !f.enabled } : f
    );
    setFlags(next); // optimistic, no reload
    const target = next.find((f) => f.key === key);
    try {
      const res = await fetch("/api/config", {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ key, enabled: target?.enabled }),
      });
      if (!res.ok) throw new Error(String(res.status));
    } catch {
      setFlags(prev); // rollback on failure
    }
  };

  if (loading) return <div aria-busy="true">Loading flags…</div>;

  return (
    <ul data-testid="admin-flags">
      {flags.map((f) => (
        <li key={f.key}>
          <span>{f.key}</span>
          <button
            role="switch"
            aria-checked={f.enabled}
            aria-label={f.key}
            onClick={() => toggle(f.key)}
          >
            {f.enabled ? "On" : "Off"}
          </button>
        </li>
      ))}
    </ul>
  );
}
