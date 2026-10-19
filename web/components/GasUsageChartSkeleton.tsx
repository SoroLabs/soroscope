'use client';

import React from 'react';

/**
 * Skeleton placeholder for {@link GasUsageChart}.
 *
 * Mirrors the chart header, legend and metric bars so the async
 * simulation fetch swaps in without layout shift (CLS). All blocks use
 * Tailwind `animate-pulse`.
 */
export const GasUsageChartSkeleton: React.FC = () => {
  return (
    <div className="rounded-lg border border-[var(--border-default)] bg-[var(--bg-card)] p-6 font-mono animate-pulse" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading gas usage chart…</span>
      <div className="flex items-center justify-between border-b-2 border-[var(--border-default)] pb-2 mb-4">
        <div className="h-6 w-52 max-w-full rounded-sm bg-[var(--bg-elevated)]" />
        <div className="h-3 w-20 rounded-sm bg-[var(--bg-elevated)]" />
      </div>
      <div className="space-y-5">
        {Array.from({ length: 5 }).map((_, index) => (
          <div key={index}>
            <div className="mb-1 flex items-center justify-between">
              <div className="h-4 w-32 rounded-sm bg-[var(--bg-elevated)]" />
              <div className="h-3 w-24 rounded-sm bg-[var(--bg-elevated)]" />
            </div>
            <div className="h-3 w-full rounded-sm bg-[var(--bg-elevated)]" />
            <div className="mt-0.5 h-2 w-full rounded-sm bg-[var(--bg-elevated)]" />
          </div>
        ))}
      </div>
    </div>
  );
};

export default GasUsageChartSkeleton;
