import React from 'react';

/**
 * Loading placeholder for `NutritionLabel`.
 *
 * The block structure mirrors the real label - masthead, headline figure,
 * five metric rows and the footnote - so the panel does not jump when the
 * analysis result arrives.
 */
export const NutritionLabelSkeleton: React.FC = () => {
  const rows = [0, 1, 2, 3, 4];

  return (
    <div className="flex flex-col gap-2">
      <div
        className="bg-[var(--bg-card)] border-2 border-[var(--border-default)] rounded-md p-4 sm:p-5 font-mono animate-pulse"
        aria-busy="true"
        aria-label="Loading nutrition label"
      >
        {/* Masthead */}
        <div className="h-7 w-56 bg-[var(--skeleton)] rounded" />
        <div className="border-b border-[var(--border-default)] pb-1 mt-2">
          <div className="h-3 w-24 bg-[var(--skeleton)] rounded" />
        </div>

        {/* Headline figure */}
        <div className="border-b-8 border-[var(--border-default)] py-2 flex items-end justify-between">
          <div className="flex flex-col gap-1.5">
            <div className="h-2.5 w-28 bg-[var(--skeleton)] rounded" />
            <div className="h-5 w-40 bg-[var(--skeleton)] rounded" />
          </div>
          <div className="h-9 w-20 bg-[var(--skeleton)] rounded" />
        </div>

        <div className="flex justify-end border-b border-[var(--border-default)] py-1">
          <div className="h-2.5 w-20 bg-[var(--skeleton)] rounded" />
        </div>

        {/* Metric rows */}
        <div>
          {rows.map((row) => (
            <div key={row} className="border-b border-[var(--border-default)] py-1.5">
              <div className="flex items-center justify-between gap-3">
                <div className="h-4 w-44 bg-[var(--skeleton)] rounded" />
                <div className="h-4 w-12 bg-[var(--skeleton)] rounded" />
              </div>
              <div className="mt-1 h-1.5 w-full bg-[var(--bg-elevated)] rounded-sm overflow-hidden">
                <div className="h-full w-1/3 bg-[var(--skeleton)]" />
              </div>
            </div>
          ))}
        </div>

        {/* Gas breakdown toggle */}
        <div className="flex items-center justify-between py-2">
          <div className="h-3 w-28 bg-[var(--skeleton)] rounded" />
          <div className="h-3 w-3 bg-[var(--skeleton)] rounded" />
        </div>

        {/* Footnote */}
        <div className="mt-1 pt-2 border-t-4 border-[var(--border-default)] flex flex-col gap-1.5">
          <div className="h-2.5 w-full bg-[var(--skeleton)] rounded" />
          <div className="h-2.5 w-4/5 bg-[var(--skeleton)] rounded" />
        </div>
      </div>

      <div className="flex justify-end">
        <div className="h-7 w-28 bg-[var(--skeleton)] rounded animate-pulse" />
      </div>
    </div>
  );
};
