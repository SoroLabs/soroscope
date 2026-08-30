'use client';

import React, { useCallback, useMemo, useRef, useState } from 'react';
import { ChevronDown, Download, Loader2 } from 'lucide-react';
import {
  buildMetrics,
  overallPercent,
  formatValue,
  formatPercent,
  type NutritionMetric,
  type NutritionResources,
  type NutritionSeverity,
} from '../lib/nutritionLabel';

interface NutritionLabelProps extends NutritionResources {
  /** Total resource fee for the transaction, used in the gas breakdown. */
  cost_stroops?: number;
  /** Optional per-deployment overrides for the protocol limits. */
  limits?: Partial<NutritionResources>;
  /** Contract function the reading belongs to, shown as the serving size. */
  functionName?: string;
}

/**
 * Bar colours per severity band. A real nutrition label is monochrome, so
 * colour is reserved for the one thing a reader must not miss: how close a
 * resource is to its ceiling.
 */
const SEVERITY_COLORS: Record<NutritionSeverity, string> = {
  low: '#2da44e',
  moderate: '#bf8700',
  high: '#e16f24',
  over: '#cf222e',
};

const SEVERITY_LABELS: Record<NutritionSeverity, string> = {
  low: 'Well within budget',
  moderate: 'Moderate usage',
  high: 'Approaching the limit',
  over: 'Over the protocol limit',
};

const stroopsToXlm = (stroops: number) => (stroops / 10_000_000).toFixed(7);

/**
 * Resource fee weighting used for the gas breakdown.
 *
 * Soroban prices each resource dimension separately, so the share a metric
 * takes of the fee is proportional to how much of its own budget it consumed.
 * Without a per-dimension fee split from the backend this is the closest
 * honest attribution, and it is labelled as an estimate in the UI.
 */
function feeShare(metric: NutritionMetric, metrics: NutritionMetric[]): number {
  const total = metrics.reduce((sum, m) => sum + m.percent, 0);
  if (total <= 0) return 0;
  return (metric.percent / total) * 100;
}

export const NutritionLabel: React.FC<NutritionLabelProps> = ({
  cpu_instructions,
  ram_bytes,
  ledger_read_bytes,
  ledger_write_bytes,
  transaction_size_bytes,
  cost_stroops,
  limits,
  functionName,
}) => {
  const labelRef = useRef<HTMLDivElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);

  const metrics = useMemo(
    () =>
      buildMetrics(
        {
          cpu_instructions,
          ram_bytes,
          ledger_read_bytes,
          ledger_write_bytes,
          transaction_size_bytes,
        },
        limits,
      ),
    [
      cpu_instructions,
      ram_bytes,
      ledger_read_bytes,
      ledger_write_bytes,
      transaction_size_bytes,
      limits,
    ],
  );

  const peak = overallPercent(metrics);
  const peakSeverity = metrics.reduce<NutritionSeverity>(
    (worst, m) => (m.percent >= peak ? m.severity : worst),
    'low',
  );

  /**
   * Export the label as a PNG. `html2canvas` is imported lazily so it stays
   * out of the initial bundle - the label renders fine without it and most
   * viewers never press export.
   */
  const handleExport = useCallback(async () => {
    if (!labelRef.current) return;
    setExporting(true);
    setExportError(null);

    try {
      const html2canvas = (await import('html2canvas')).default;
      const canvas = await html2canvas(labelRef.current, {
        backgroundColor: getComputedStyle(labelRef.current).backgroundColor || '#ffffff',
        scale: 2,
        logging: false,
      });

      const dataUrl = canvas.toDataURL('image/png');
      const link = document.createElement('a');
      link.href = dataUrl;
      link.download = `soroscope-nutrition-${functionName || 'transaction'}-${Date.now()}.png`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
    } catch (error) {
      // A failed export must never take the label down with it.
      console.error('Nutrition label PNG export failed:', error);
      setExportError('Could not export the label as a PNG.');
    } finally {
      setExporting(false);
    }
  }, [functionName]);

  return (
    <div className="flex flex-col gap-2">
      <div
        ref={labelRef}
        className="bg-[var(--bg-card)] border-2 border-[var(--text-primary)] rounded-md p-4 sm:p-5 font-mono text-[var(--text-primary)]"
      >
        {/* Masthead */}
        <h2 className="text-2xl sm:text-3xl font-black uppercase tracking-tight leading-none">
          Nutrition Facts
        </h2>
        <div className="border-b border-[var(--text-primary)] pb-1 mt-1 text-xs text-[var(--text-secondary)]">
          1 transaction{functionName ? ` (${functionName})` : ''}
        </div>

        {/* Headline figure */}
        <div className="border-b-8 border-[var(--text-primary)] py-1 flex items-end justify-between">
          <div>
            <div className="text-[10px] uppercase tracking-wide text-[var(--text-secondary)]">
              Amount per transaction
            </div>
            <div className="text-lg font-black uppercase">Peak Resource Load</div>
          </div>
          <div
            className="text-3xl sm:text-4xl font-black tabular-nums"
            style={{ color: SEVERITY_COLORS[peakSeverity] }}
          >
            {formatPercent(peak)}
          </div>
        </div>

        <div className="text-[10px] text-right font-bold uppercase tracking-wide border-b border-[var(--text-primary)] py-1">
          % Daily Value*
        </div>

        {/* Metric rows */}
        <ul className="list-none m-0 p-0">
          {metrics.map((metric) => (
            <li key={metric.key} className="border-b border-[var(--border-default)] py-1.5">
              <div className="flex items-baseline justify-between gap-3">
                <span className={metric.bold ? 'font-bold text-sm' : 'text-sm pl-3'}>
                  {metric.label}{' '}
                  <span className="font-normal text-[var(--text-secondary)]">
                    {formatValue(metric.value)} {metric.unit}
                  </span>
                </span>
                <span
                  className="font-bold text-sm tabular-nums shrink-0"
                  style={{ color: SEVERITY_COLORS[metric.severity] }}
                  title={SEVERITY_LABELS[metric.severity]}
                >
                  {formatPercent(metric.percent)}
                </span>
              </div>

              <div
                className="mt-1 h-1.5 w-full bg-[var(--bg-elevated)] rounded-sm overflow-hidden"
                role="meter"
                aria-valuenow={Math.round(metric.percent)}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-label={`${metric.label}: ${formatPercent(metric.percent)} of the protocol limit`}
              >
                <div
                  className="h-full transition-all duration-500 ease-out"
                  style={{
                    width: `${metric.barPercent}%`,
                    backgroundColor: SEVERITY_COLORS[metric.severity],
                  }}
                />
              </div>
            </li>
          ))}
        </ul>

        {/* Expandable gas breakdown */}
        <button
          type="button"
          onClick={() => setExpanded((open) => !open)}
          aria-expanded={expanded}
          aria-controls="nutrition-gas-breakdown"
          className="mt-2 w-full flex items-center justify-between text-xs font-bold uppercase tracking-wide py-1.5 text-[var(--text-primary)] hover:text-[var(--tab-hover)] transition-colors"
        >
          <span>Gas Breakdown</span>
          <ChevronDown
            size={14}
            className={`transition-transform duration-200 ${expanded ? 'rotate-180' : ''}`}
            aria-hidden="true"
          />
        </button>

        {expanded && (
          <div
            id="nutrition-gas-breakdown"
            className="border-t border-[var(--border-default)] pt-2 text-xs"
          >
            {typeof cost_stroops === 'number' && (
              <div className="flex justify-between font-bold pb-1.5 mb-1.5 border-b border-[var(--border-default)]">
                <span>Total resource fee</span>
                <span className="tabular-nums">{stroopsToXlm(cost_stroops)} XLM</span>
              </div>
            )}

            <table className="w-full border-collapse">
              <thead>
                <tr className="text-[10px] uppercase text-[var(--text-secondary)] text-left">
                  <th scope="col" className="font-normal pb-1">
                    Resource
                  </th>
                  <th scope="col" className="font-normal pb-1 text-right">
                    Used
                  </th>
                  <th scope="col" className="font-normal pb-1 text-right">
                    Limit
                  </th>
                  <th scope="col" className="font-normal pb-1 text-right">
                    Share
                  </th>
                </tr>
              </thead>
              <tbody>
                {metrics.map((metric) => (
                  <tr key={metric.key} className="align-baseline">
                    <td className="py-0.5 pr-2">{metric.label}</td>
                    <td className="py-0.5 text-right tabular-nums">{formatValue(metric.value)}</td>
                    <td className="py-0.5 text-right tabular-nums text-[var(--text-secondary)]">
                      {formatValue(metric.limit)}
                    </td>
                    <td className="py-0.5 text-right tabular-nums">
                      {formatPercent(feeShare(metric, metrics))}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>

            <p className="mt-2 text-[10px] leading-snug text-[var(--text-secondary)]">
              Share is each resource&apos;s portion of the total budget consumed, which
              approximates its contribution to the resource fee. Soroban prices each dimension
              separately, so treat it as an estimate.
            </p>
          </div>
        )}

        {/* Footnote */}
        <p className="mt-3 pt-2 border-t-4 border-[var(--text-primary)] text-[10px] leading-snug text-[var(--text-secondary)]">
          * Percent Daily Value is the share of the Soroban per-transaction resource limit used by
          this call. Limits vary by protocol version.
        </p>
      </div>

      {/* Export control, kept outside the captured area */}
      <div className="flex items-center justify-end gap-2">
        {exportError && (
          <span role="alert" className="text-xs text-[#cf222e]">
            {exportError}
          </span>
        )}
        <button
          type="button"
          onClick={handleExport}
          disabled={exporting}
          className="inline-flex items-center gap-1.5 rounded border border-[var(--border-default)] bg-[var(--bg-elevated)] px-2.5 py-1.5 text-xs font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-card)] disabled:cursor-not-allowed disabled:opacity-60"
        >
          {exporting ? (
            <Loader2 size={14} className="animate-spin" aria-hidden="true" />
          ) : (
            <Download size={14} aria-hidden="true" />
          )}
          {exporting ? 'Exporting...' : 'Export PNG'}
        </button>
      </div>
    </div>
  );
};
