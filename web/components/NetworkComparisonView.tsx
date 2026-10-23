import { useEffect, useState } from 'react';

import { NETWORKS, type NetworkId } from '../context/NetworkContext';
import { analyzeAcrossNetworks, type NetworkAnalysisOutcome } from '../lib/api';
import {
  DEFAULT_COMPARE_NETWORKS,
  DIFF_THRESHOLD,
  buildComparisonRows,
  formatDifference,
} from '../lib/networkComparison';

interface NetworkComparisonViewProps {
  contractId: string;
  functionName: string;
  args: string[];
  /** Changes whenever a new simulation should be compared (e.g. result id). */
  runKey: string;
  networkIds?: string[];
}

type Status = 'loading' | 'done';

function networkName(id: string): string {
  return NETWORKS[id as NetworkId]?.shortName ?? id;
}

function formatValue(value: number | null): string {
  return value === null ? '—' : value.toLocaleString();
}

/**
 * Runs one simulation per network in parallel and shows the results in a
 * two-column diff, highlighting metrics that differ by more than 5% (#1282).
 */
export function NetworkComparisonView({
  contractId,
  functionName,
  args,
  runKey,
  networkIds = DEFAULT_COMPARE_NETWORKS,
}: NetworkComparisonViewProps) {
  const [status, setStatus] = useState<Status>('loading');
  const [outcomes, setOutcomes] = useState<NetworkAnalysisOutcome[]>([]);
  const [left, right] = networkIds;
  const argsKey = JSON.stringify(args);

  useEffect(() => {
    let cancelled = false;
    setStatus('loading');
    setOutcomes([]);

    analyzeAcrossNetworks(
      { contract_id: contractId, function_name: functionName, args: JSON.parse(argsKey) },
      [left, right],
    ).then((results) => {
      if (cancelled) return;
      setOutcomes(results);
      setStatus('done');
    });

    return () => {
      cancelled = true;
    };
  }, [contractId, functionName, argsKey, left, right, runKey]);

  const leftOutcome = outcomes.find((o) => o.networkId === left);
  const rightOutcome = outcomes.find((o) => o.networkId === right);
  const rows = buildComparisonRows(leftOutcome?.report, rightOutcome?.report);
  const differing = rows.filter((r) => r.differs).length;
  const thresholdPct = Math.round(DIFF_THRESHOLD * 100);

  const headerCell = (id: string, outcome?: NetworkAnalysisOutcome) => (
    <th scope="col" className="px-3 py-2 font-medium">
      <div className="text-[var(--text-primary)]">{networkName(id)}</div>
      {outcome?.error ? (
        <div className="mt-0.5 text-[11px] font-normal text-[#f0883e]" role="alert">
          {outcome.error}
        </div>
      ) : null}
    </th>
  );

  return (
    <section
      aria-label="Network comparison"
      aria-busy={status === 'loading'}
      className="mt-4 rounded-lg border border-[var(--border-default)] bg-[var(--bg-card)]"
    >
      <div className="flex flex-wrap items-baseline justify-between gap-2 border-b border-[var(--border-default)] px-3 py-2">
        <h4 className="text-sm font-semibold text-[var(--text-primary)]">
          {networkName(left)} vs {networkName(right)}
        </h4>
        <p className="text-xs text-[var(--text-secondary)]">
          {status === 'loading'
            ? 'Running both simulations…'
            : `${differing} metric${differing === 1 ? '' : 's'} differ by more than ${thresholdPct}%`}
        </p>
      </div>

      <div className="overflow-x-auto">
        <table className="min-w-full text-left text-xs">
          <thead className="text-[var(--text-secondary)]">
            <tr>
              <th scope="col" className="px-3 py-2 font-medium">
                Metric
              </th>
              {headerCell(left, leftOutcome)}
              {headerCell(right, rightOutcome)}
              <th scope="col" className="px-3 py-2 font-medium">
                Difference
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-[var(--border-default)] font-mono">
            {rows.map((row) => {
              const cellClass = row.differs
                ? 'px-3 py-2 bg-amber-500/15 text-amber-200'
                : 'px-3 py-2 text-[var(--text-primary)]';
              return (
                <tr key={row.key} data-differs={row.differs ? 'true' : 'false'}>
                  <th scope="row" className="px-3 py-2 font-sans font-normal text-[var(--text-secondary)]">
                    {row.label}
                  </th>
                  <td className={cellClass}>{status === 'loading' ? '…' : formatValue(row.left)}</td>
                  <td className={cellClass}>{status === 'loading' ? '…' : formatValue(row.right)}</td>
                  <td className={cellClass}>
                    {status === 'loading' ? '…' : formatDifference(row.difference)}
                    {row.differs ? <span className="sr-only"> (exceeds {thresholdPct}%)</span> : null}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}
