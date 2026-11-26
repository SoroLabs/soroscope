export interface ComparisonMetric {
  key: string;
  label: string;
}

export interface ComparisonRow {
  key: string;
  label: string;
  left: number | null;
  right: number | null;
  /** Relative difference in [0, 1], or null when a side is missing. */
  difference: number | null;
  /** True when the values differ by more than the threshold (5% by default). */
  differs: boolean;
}

export const COMPARISON_METRICS: ComparisonMetric[];
export const DEFAULT_COMPARE_NETWORKS: string[];
export const DIFF_THRESHOLD: number;

export function relativeDifference(a: unknown, b: unknown): number | null;
export function buildComparisonRows(
  left: object | null | undefined,
  right: object | null | undefined,
  threshold?: number,
): ComparisonRow[];
export function formatDifference(difference: number | null | undefined): string;
