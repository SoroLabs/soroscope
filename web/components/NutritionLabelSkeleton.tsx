'use client';

import React from 'react';

/**
 * Skeleton placeholder for {@link NutritionLabel}.
 *
 * Mirrors the label's masthead, headline figure, metric rows and gas
 * breakdown toggle so the async simulation fetch swaps in without any
 * layout shift (CLS). Every block uses the same spacing, borders and
 * heights as the loaded component; only the text is replaced by pulsing
 * bars.
 */
export const NutritionLabelSkeleton: React.FC = () => {
  return (
    <div className="flex flex-col gap-2" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading nutrition label…</span>
      <div className="bg-[var(--bg-card)] border-2 border-[var(--text-primary)] rounded-md p-4 sm:p-5 font-mono text-[var(--text-primary)] animate-pulse">
        {/* Masthead */}
        <div className="h-7 sm:h-9 w-56 max-w-full rounded-sm bg-[var(--bg-elevated)]" />
        <div className="border-b border-[var(--text-primary)] pb-1 mt-1">
          <div className="h-3 w-32 rounded-sm bg-[var(--bg-elevated)]" />
        </div>

        {/* Headline figure */}
        <div className="border-b-8 border-[var(--text-primary)] py-1 flex items-end justify-between">
          <div className="flex flex-col gap-1">
            <div className="h-2.5 w-40 rounded-sm bg-[var(--bg-elevated)]" />
            <div className="h-5 w-44 rounded-sm bg-[var(--bg-elevated)]" />
          </div>
          <div className="h-9 sm:h-10 w-20 rounded-sm bg-[var(--bg-elevated)]" />
        </div>

        <div className="border-b border-[var(--text-primary)] py-1 flex justify-end">
          <div className="h-2.5 w-24 rounded-sm bg-[var(--bg-elevated)]" />
        </div>

        {/* Metric rows */}
        <ul className="list-none m-0 p-0">
          {Array.from({ length: 5 }).map((_, index) => (
            <li key={index} className="border-b border-[var(--border-default)] py-1.5">
              <div className="flex items-baseline justify-between gap-3">
                <div className="h-4 w-40 max-w-[60%] rounded-sm bg-[var(--bg-elevated)]" />
                <div className="h-4 w-12 shrink-0 rounded-sm bg-[var(--bg-elevated)]" />
              </div>
              <div className="mt-1 h-1.5 w-full bg-[var(--bg-elevated)] rounded-sm overflow-hidden">
                <div className="h-full w-1/3 bg-[var(--border-default)]" />
              </div>
            </li>
          ))}
        </ul>

        {/* Gas breakdown toggle */}
        <div className="mt-2 w-full flex items-center justify-between py-1.5">
          <div className="h-3 w-28 rounded-sm bg-[var(--bg-elevated)]" />
          <div className="h-3.5 w-3.5 rounded-sm bg-[var(--bg-elevated)]" />
        </div>
      </div>
    </div>
  );
};

export default NutritionLabelSkeleton;
