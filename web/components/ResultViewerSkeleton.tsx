'use client';

export function ResultViewerSkeleton() {
  const heatmapCells = Array.from({ length: 36 }, (_, index) => {
    const phase = index % 4;
    const toneClass =
      phase === 0
        ? 'bg-cyan-600/30 border-cyan-500/20'
        : phase === 1
        ? 'bg-slate-700/70 border-slate-600/60'
        : phase === 2
        ? 'bg-amber-500/20 border-amber-500/30'
        : 'bg-slate-800/80 border-slate-700/80';
    return { id: index, toneClass };
  });

  return (
    <div
      style={{
        padding: '24px',
        backgroundColor: 'var(--bg-elevated)',
        borderRadius: '8px',
        borderLeft: '4px solid #00d9ff',
        border: '1px solid #30363d',
      }}
      className="animate-pulse"
    >
      <div style={{ marginBottom: '24px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div className="flex items-center gap-3">
          <div className="relative flex h-3 w-3">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#00d9ff] opacity-75"></span>
            <span className="relative inline-flex rounded-full h-3 w-3 bg-[#00d9ff]"></span>
          </div>
          <div>
            <h3
              style={{
                margin: '0 0 4px 0',
                color: '#00d9ff',
                fontSize: '16px',
                fontWeight: '600',
              }}
            >
              Simulating Transaction...
            </h3>
            <p style={{ margin: '0', color: 'var(--text-secondary)', fontSize: '12px' }}>
              Profiling smart contract resource cost
            </p>
          </div>
        </div>

        <div className="h-8 w-40 bg-[#1f2937] rounded-md border border-[#374151]" />
      </div>

      {/* Code Result Skeleton Box */}
      <div
        style={{
          backgroundColor: 'var(--bg-elevated)',
          padding: '16px',
          borderRadius: '6px',
          marginBottom: '16px',
          border: '1px solid #30363d',
        }}
      >
        <div className="flex flex-col gap-3">
          <div className="h-4 w-24 bg-[var(--skeleton)] rounded" />
          <div className="h-3 w-full bg-[var(--bg-card)] rounded" />
          <div className="h-3 w-5/6 bg-[var(--bg-card)] rounded" />
          <div className="h-3 w-4/5 bg-[var(--bg-card)] rounded" />
          <div className="h-3 w-2/3 bg-[var(--bg-card)] rounded" />
        </div>
      </div>

      {/* Call Graph Skeleton Box */}
      <div
        style={{
          backgroundColor: 'var(--bg-card)',
          padding: '20px',
          borderRadius: '8px',
          border: '1px solid var(--border-default)',
          minHeight: '120px',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          gap: '12px',
        }}
      >
        <div className="h-4 w-32 bg-[var(--skeleton)] rounded mb-2" />
        <div className="flex items-center gap-4">
          <div className="h-10 w-24 bg-[var(--bg-elevated)] rounded-lg border border-[var(--border-default)] flex items-center justify-center">
            <div className="h-2 w-12 bg-[var(--skeleton)] rounded" />
          </div>
          <div className="h-[2px] w-8 bg-[var(--skeleton)] relative">
            <div className="absolute right-0 top-1/2 -translate-y-1/2 border-t-[4px] border-b-[4px] border-l-[6px] border-transparent border-l-[#30363d]" />
          </div>
          <div className="h-10 w-24 bg-[var(--bg-elevated)] rounded-lg border border-[var(--border-default)] flex items-center justify-center">
            <div className="h-2 w-12 bg-[var(--skeleton)] rounded" />
          </div>
        </div>
      </div>

      {/* Heatmap Matrix Skeleton */}
      <div
        style={{
          backgroundColor: 'var(--bg-card)',
          padding: '20px',
          borderRadius: '8px',
          border: '1px solid var(--border-default)',
          marginTop: '16px',
        }}
      >
        <div className="flex items-center justify-between mb-4">
          <div className="h-4 w-44 bg-[var(--skeleton)] rounded" />
          <div className="h-7 w-32 bg-[var(--bg-elevated)] rounded-md border border-[var(--border-default)]" />
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-[auto_1fr] gap-6">
          <div className="grid grid-cols-6 gap-2 bg-[var(--bg-elevated)] p-3 rounded-lg border border-[var(--border-default)] w-fit">
            {heatmapCells.map((cell) => (
              <div
                key={cell.id}
                className={`w-8 h-8 rounded border ${cell.toneClass}`}
              />
            ))}
          </div>

          <div className="bg-[var(--bg-elevated)] rounded-lg border border-[var(--border-default)] p-4 min-h-[170px]">
            <div className="h-3 w-48 bg-[var(--skeleton)] rounded mb-4" />
            <div className="h-5 w-36 bg-[#1f2937] rounded mb-3" />
            <div className="h-3 w-full bg-[#1f2937] rounded mb-2" />
            <div className="h-3 w-11/12 bg-[#1f2937] rounded mb-2" />
            <div className="h-3 w-9/12 bg-[#1f2937] rounded" />
          </div>
        </div>

        <div className="mt-4 grid grid-cols-2 md:grid-cols-4 gap-3">
          <div className="h-14 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-default)]" />
          <div className="h-14 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-default)]" />
          <div className="h-14 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-default)]" />
          <div className="h-14 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-default)]" />
        </div>
      </div>
    </div>
  );
}
