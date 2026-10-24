/**
 * Side-by-side multi-network simulation comparison (#1282).
 *
 * Pure helpers so the diff rules can be unit tested without React.
 */

/** Networks compared when the "Compare networks" toggle is on. */
const DEFAULT_COMPARE_NETWORKS = ['testnet', 'futurenet'];

/** Cells are highlighted when values differ by more than this fraction. */
const DIFF_THRESHOLD = 0.05;

const COMPARISON_METRICS = [
  { key: 'cpu_instructions', label: 'CPU instructions' },
  { key: 'ram_bytes', label: 'RAM (bytes)' },
  { key: 'ledger_read_bytes', label: 'Ledger read (bytes)' },
  { key: 'ledger_write_bytes', label: 'Ledger write (bytes)' },
  { key: 'transaction_size_bytes', label: 'Transaction size (bytes)' },
  { key: 'cost_stroops', label: 'Fee (stroops)' },
];

function toNumber(value) {
  if (typeof value === 'number') return Number.isFinite(value) ? value : null;
  if (typeof value === 'string' && value.trim() !== '') {
    const n = Number(value);
    return Number.isFinite(n) ? n : null;
  }
  return null;
}

/**
 * Relative difference between two values, measured against the larger of the
 * two so the result is symmetric (A vs B equals B vs A) and bounded to [0, 1].
 * Returns null when either side is missing.
 */
function relativeDifference(a, b) {
  const left = toNumber(a);
  const right = toNumber(b);
  if (left === null || right === null) return null;
  const scale = Math.max(Math.abs(left), Math.abs(right));
  if (scale === 0) return 0;
  return Math.abs(left - right) / scale;
}

/**
 * Build one row per metric with both networks' values and whether the pair
 * differs by more than `threshold`.
 *
 * @param {Record<string, unknown> | null | undefined} left
 * @param {Record<string, unknown> | null | undefined} right
 * @param {number} [threshold]
 */
function buildComparisonRows(left, right, threshold = DIFF_THRESHOLD) {
  return COMPARISON_METRICS.map(({ key, label }) => {
    const leftValue = left ? toNumber(left[key]) : null;
    const rightValue = right ? toNumber(right[key]) : null;
    const difference = relativeDifference(leftValue, rightValue);
    return {
      key,
      label,
      left: leftValue,
      right: rightValue,
      difference,
      differs: difference !== null && difference > threshold,
    };
  });
}

function formatDifference(difference) {
  if (difference === null || difference === undefined) return '—';
  return `${(difference * 100).toFixed(1)}%`;
}

module.exports = {
  COMPARISON_METRICS,
  DEFAULT_COMPARE_NETWORKS,
  DIFF_THRESHOLD,
  buildComparisonRows,
  formatDifference,
  relativeDifference,
};
