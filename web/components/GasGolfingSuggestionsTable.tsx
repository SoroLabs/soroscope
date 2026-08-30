'use client';

import React, { useMemo, useState } from 'react';
import clsx from 'clsx';
import { sortGasGolfingSuggestions } from '../lib/gasGolfingSort';
import type {
  GasGolfingSortKey,
  GasGolfingSuggestion,
  SortDirection,
} from '../lib/gasGolfingSort';

function severityChip(severity: string) {
  const normalized = severity.toLowerCase();
  const style =
    normalized === 'high'
      ? 'border-red-500/50 bg-red-500/10 text-red-200'
      : normalized === 'medium'
        ? 'border-yellow-500/50 bg-yellow-500/10 text-yellow-200'
        : normalized === 'low'
          ? 'border-emerald-500/50 bg-emerald-500/10 text-emerald-200'
          : 'border-slate-500/50 bg-slate-500/10 text-slate-200';

  return (
    <span
      className={clsx(
        'inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] font-semibold',
        style,
      )}
    >
      {severity.toUpperCase()}
    </span>
  );
}

function SortIndicator({
  active,
  direction,
}: {
  active: boolean;
  direction: SortDirection;
}) {
  if (!active) return <span className="ml-1 text-xs text-[var(--text-muted)]">↕</span>;
  return (
    <span className="ml-1 text-xs text-[#00d9ff]">
      {direction === 'asc' ? '↑' : '↓'}
    </span>
  );
}

export function GasGolfingSuggestionsTable({
  suggestions,
}: {
  suggestions: GasGolfingSuggestion[];
}) {
  const [sortKey, setSortKey] = useState<GasGolfingSortKey>('severity');
  const [direction, setDirection] = useState<SortDirection>('desc');

  const sorted = useMemo(
    () => sortGasGolfingSuggestions(suggestions, sortKey, direction),
    [suggestions, sortKey, direction],
  );

  const toggleSort = (key: GasGolfingSortKey) => {
    if (key !== sortKey) {
      setSortKey(key);
      setDirection('desc');
      return;
    }
    setDirection((d) => (d === 'desc' ? 'asc' : 'desc'));
  };

  if (!suggestions.length) {
    return (
      <div className="rounded-lg border border-[var(--border-default)] bg-[var(--bg-elevated)] p-4 text-sm text-[var(--text-secondary)]">
        No gas golfing suggestions found.
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-[var(--border-default)] bg-[var(--bg-elevated)]">
      <div className="flex items-center justify-between border-b border-[var(--border-default)] px-4 py-3">
        <div>
          <h3 className="text-sm font-semibold text-[var(--text-primary)]">
            Gas Golfing Suggestions
          </h3>
          <p className="mt-0.5 text-xs text-[var(--text-secondary)]">
            Click a column header to sort.
          </p>
        </div>
        <div className="text-xs text-[var(--text-secondary)]">
          {sorted.length} suggestion{sorted.length === 1 ? '' : 's'}
        </div>
      </div>

      <div className="overflow-x-auto">
        <table className="min-w-full text-left text-sm">
          <thead className="bg-[var(--bg-card)] text-xs text-[var(--text-secondary)]">
            <tr>
              <th className="px-4 py-3 font-medium">Suggestion</th>
              <th className="px-4 py-3 font-medium">
                <button
                  type="button"
                  onClick={() => toggleSort('severity')}
                  className="inline-flex items-center hover:text-[var(--text-primary)]"
                >
                  Severity
                  <SortIndicator
                    active={sortKey === 'severity'}
                    direction={direction}
                  />
                </button>
              </th>
              <th className="px-4 py-3 font-medium">
                <button
                  type="button"
                  onClick={() => toggleSort('gas_saved_estimate')}
                  className="inline-flex items-center hover:text-[var(--text-primary)]"
                >
                  Gas Saved
                  <SortIndicator
                    active={sortKey === 'gas_saved_estimate'}
                    direction={direction}
                  />
                </button>
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-[var(--border-default)]">
            {sorted.map((s, idx) => (
              <tr key={`${s.title}-${idx}`} className="hover:bg-[var(--bg-card)]">
                <td className="px-4 py-3">
                  <div className="font-medium text-[var(--text-primary)]">{s.title}</div>
                  {s.description ? (
                    <div className="mt-0.5 text-xs text-[var(--text-secondary)]">
                      {s.description}
                    </div>
                  ) : null}
                </td>
                <td className="px-4 py-3">{severityChip(String(s.severity))}</td>
                <td className="px-4 py-3 font-mono text-xs text-[var(--text-primary)]">
                  {s.gas_saved_estimate ?? '—'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

