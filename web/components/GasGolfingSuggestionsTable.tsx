'use client';

import React, { useMemo, useState } from 'react';
import clsx from 'clsx';
import {
  normalizeGasGolfingSuggestions,
  sortGasGolfingSuggestions,
} from '../lib/gasGolfingSort';
import type {
  GasGolfingSortKey,
  GasGolfingSuggestion,
  SortDirection,
} from '../lib/gasGolfingSort';

const SEVERITY_FILTERS = ['High', 'Medium', 'Low', 'Info'] as const;
type SeverityFilter = (typeof SEVERITY_FILTERS)[number];

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

function suggestionFixText(s: GasGolfingSuggestion): string {
  const parts = [s.title];
  if (s.description) parts.push(s.description);
  if (s.recommendation) parts.push(s.recommendation);
  return parts.join('\n\n');
}

export function GasGolfingSuggestionsTable({
  suggestions,
}: {
  suggestions: GasGolfingSuggestion[];
}) {
  const [sortKey, setSortKey] = useState<GasGolfingSortKey>('severity');
  const [direction, setDirection] = useState<SortDirection>('desc');
  const [activeFilters, setActiveFilters] = useState<SeverityFilter[]>([]);
  const [preview, setPreview] = useState<GasGolfingSuggestion | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  // Normalizing is idempotent: rows that already have an id keep it, and raw
  // server rows (or ones from an older server without ids) get a unique one.
  const rows = useMemo(() => normalizeGasGolfingSuggestions(suggestions), [suggestions]);

  const filtered = useMemo(() => {
    if (!activeFilters.length) return rows;
    const wanted = new Set(activeFilters.map((f) => f.toLowerCase()));
    return rows.filter((s) => wanted.has(String(s.severity).toLowerCase()));
  }, [rows, activeFilters]);

  const sorted = useMemo(
    () => sortGasGolfingSuggestions(filtered, sortKey, direction),
    [filtered, sortKey, direction],
  );

  const toggleSort = (key: GasGolfingSortKey) => {
    if (key !== sortKey) {
      setSortKey(key);
      setDirection('desc');
      return;
    }
    setDirection((d) => (d === 'desc' ? 'asc' : 'desc'));
  };

  const toggleFilter = (filter: SeverityFilter) => {
    setActiveFilters((current) =>
      current.includes(filter)
        ? current.filter((f) => f !== filter)
        : [...current, filter],
    );
  };

  const copySuggestion = async (s: GasGolfingSuggestion, key: string) => {
    const text = suggestionFixText(s);
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard) {
        await navigator.clipboard.writeText(text);
      }
      setCopiedKey(key);
      window.setTimeout(() => setCopiedKey((k) => (k === key ? null : k)), 1500);
    } catch {
      setCopiedKey(null);
    }
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

      <div className="flex flex-wrap items-center gap-2 border-b border-[var(--border-default)] px-4 py-3">
        {SEVERITY_FILTERS.map((filter) => {
          const active = activeFilters.includes(filter);
          return (
            <button
              key={filter}
              type="button"
              onClick={() => toggleFilter(filter)}
              aria-pressed={active}
              aria-label={`Filter by ${filter} severity`}
              className={clsx(
                'rounded-full border px-3 py-1 text-xs font-semibold transition-colors',
                active
                  ? 'border-[#00d9ff] bg-[#00d9ff]/10 text-[#00d9ff]'
                  : 'border-[var(--border-default)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]',
              )}
            >
              {filter}
            </button>
          );
        })}
        {activeFilters.length ? (
          <button
            type="button"
            onClick={() => setActiveFilters([])}
            aria-label="Clear all severity filters"
            className="ml-1 text-xs text-[var(--text-secondary)] underline hover:text-[var(--text-primary)]"
          >
            Clear
          </button>
        ) : null}
      </div>

      <div className="overflow-x-auto">
        <table className="min-w-full text-left text-sm" aria-label="Gas golfing optimization suggestions">
          <thead className="bg-[var(--bg-card)] text-xs text-[var(--text-secondary)]">
            <tr>
              <th scope="col" className="px-4 py-3 font-medium">Suggestion</th>
              <th scope="col" className="px-4 py-3 font-medium">
                <button
                  type="button"
                  onClick={() => toggleSort('severity')}
                  aria-label="Sort by severity"
                  aria-sort={sortKey === 'severity' ? (direction === 'asc' ? 'ascending' : 'descending') : 'none'}
                  className="inline-flex items-center hover:text-[var(--text-primary)]"
                >
                  Severity
                  <SortIndicator
                    active={sortKey === 'severity'}
                    direction={direction}
                  />
                </button>
              </th>
              <th scope="col" className="px-4 py-3 font-medium">
                <button
                  type="button"
                  onClick={() => toggleSort('gas_saved_estimate')}
                  aria-label="Sort by estimated gas saved"
                  aria-sort={sortKey === 'gas_saved_estimate' ? (direction === 'asc' ? 'ascending' : 'descending') : 'none'}
                  className="inline-flex items-center hover:text-[var(--text-primary)]"
                >
                  Gas Saved
                  <SortIndicator
                    active={sortKey === 'gas_saved_estimate'}
                    direction={direction}
                  />
                </button>
              </th>
              <th scope="col" className="px-4 py-3 font-medium">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-[var(--border-default)]">
            {sorted.map((s) => {
              // Key on the suggestion's stable id, never the array index, so
              // re-sorting or re-fetching can't make React reuse the wrong
              // row and render duplicates (#1285).
              const rowKey = s.id;
              return (
                <tr key={rowKey} data-suggestion-id={s.id} className="hover:bg-[var(--bg-card)]">
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
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      <button
                        type="button"
                        onClick={() => setPreview(s)}
                        className="rounded border border-[var(--border-default)] px-2 py-1 text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                      >
                        Preview
                      </button>
                      <button
                        type="button"
                        onClick={() => copySuggestion(s, rowKey)}
                        className="rounded border border-[var(--border-default)] px-2 py-1 text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                      >
                        {copiedKey === rowKey ? 'Copied!' : 'Copy Suggestion'}
                      </button>
                    </div>
                  </td>
                </tr>
              );
            })}
            {!sorted.length ? (
              <tr>
                <td
                  colSpan={4}
                  className="px-4 py-6 text-center text-sm text-[var(--text-secondary)]"
                >
                  No suggestions match the selected filters.
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>

      {preview ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4"
          onKeyDown={(e) => {
            if (e.key === 'Escape') setPreview(null);
          }}
        >
          {/* Backdrop */}
          <div
            className="fixed inset-0 bg-black/60 backdrop-blur-sm"
            onClick={() => setPreview(null)}
            aria-hidden="true"
          />

          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="gas-golfing-preview-title"
            aria-describedby={preview.description ? "gas-golfing-preview-desc" : undefined}
            tabIndex={-1}
            className="relative z-10 w-full max-w-lg rounded-lg border border-[var(--border-default)] bg-[var(--bg-elevated)] p-4 shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-start justify-between gap-4">
              <div>
                <h4 id="gas-golfing-preview-title" className="text-sm font-semibold text-[var(--text-primary)]">
                  {preview.title}
                </h4>
                <div className="mt-1">{severityChip(String(preview.severity))}</div>
              </div>
              <button
                type="button"
                onClick={() => setPreview(null)}
                aria-label="Close suggestion preview"
                className="text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
              >
                Close
              </button>
            </div>
            {preview.description ? (
              <p id="gas-golfing-preview-desc" className="mt-3 text-xs text-[var(--text-secondary)]">
                {preview.description}
              </p>
            ) : null}
            <pre className="mt-3 max-h-64 overflow-auto rounded border border-[var(--border-default)] bg-[var(--bg-card)] p-3 text-xs text-[var(--text-primary)]">
              <code>{preview.recommendation ?? 'No recommended fix provided.'}</code>
            </pre>
            <div className="mt-3 flex justify-end">
              <button
                type="button"
                onClick={() => copySuggestion(preview, `preview-${preview.id}`)}
                aria-label={`Copy suggestion fix: ${preview.title}`}
                className="rounded border border-[var(--border-default)] px-3 py-1 text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
              >
                {copiedKey === `preview-${preview.id}` ? 'Copied!' : 'Copy Suggestion'}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
