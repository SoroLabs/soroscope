'use client';

import React from 'react';

/**
 * Skeleton placeholder for {@link CallGraphVisualizer}.
 *
 * Mirrors the graph header, canvas area and inspector hint so the async
 * call-graph fetch swaps in without layout shift (CLS). All blocks use
 * Tailwind `animate-pulse`.
 */
export const CallGraphVisualizerSkeleton: React.FC = () => {
  return (
    <div className="mt-5 animate-pulse" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading call graph…</span>
      <div className="mb-3 flex items-center justify-between gap-2">
        <div className="h-4 w-48 rounded-sm bg-[var(--bg-elevated)]" />
        <div className="h-3 w-24 rounded-sm bg-[var(--bg-elevated)]" />
      </div>
      <div className="h-[380px] w-full rounded-lg border border-[var(--border-default)] bg-[var(--bg-elevated)]" />
      <div className="mt-2 h-3 w-64 max-w-full rounded-sm bg-[var(--bg-elevated)]" />
    </div>
  );
};

export default CallGraphVisualizerSkeleton;
