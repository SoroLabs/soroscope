'use client';

import React from 'react';

/**
 * Skeleton placeholder for {@link ResourceHeatmap}.
 *
 * Mirrors the header, tab bar and gauge grid so the async simulation
 * fetch swaps in without layout shift (CLS). All blocks use Tailwind
 * `animate-pulse`.
 */
export const ResourceHeatmapSkeleton: React.FC = () => {
  return (
    <div className="w-full rounded-xl border border-slate-800 bg-slate-900/90 p-6 animate-pulse" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading resource heatmap…</span>
      <div className="flex items-center justify-between border-b border-slate-800/80 pb-5 mb-6">
        <div className="h-5 w-56 max-w-full rounded-sm bg-slate-800" />
        <div className="h-8 w-44 rounded-md bg-slate-800" />
      </div>
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {Array.from({ length: 3 }).map((_, index) => (
          <div key={index} className="flex flex-col items-center rounded-xl border border-slate-800/60 bg-slate-950/40 p-5">
            <div className="h-32 w-32 rounded-full bg-slate-800" />
            <div className="mt-4 h-3 w-24 rounded-sm bg-slate-800" />
          </div>
        ))}
      </div>
    </div>
  );
};

export default ResourceHeatmapSkeleton;
