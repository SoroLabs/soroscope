// Pure calculation helpers for the contract nutrition label.
//
// The component renders a nutritional-facts style panel, so the numbers it
// shows are "percent daily values" measured against the Soroban per-transaction
// resource limits. Keeping the maths here (rather than inline in the component)
// makes it directly unit-testable and keeps the JSX declarative.

/**
 * Soroban per-transaction resource ceilings.
 *
 * These mirror the network protocol limits the backend validates against.
 * They are exported so callers can override individual entries when a
 * deployment runs with non-default limits.
 */
const SOROBAN_LIMITS = Object.freeze({
  cpu_instructions: 100_000_000,
  ram_bytes: 40 * 1024 * 1024,
  ledger_read_bytes: 64 * 1024,
  ledger_write_bytes: 64 * 1024,
  transaction_size_bytes: 128 * 1024,
});

/**
 * Ordered metric descriptors. Order drives the render order of the label.
 * `key` matches the field names used by the `/analyze` response.
 */
const METRIC_DEFINITIONS = Object.freeze([
  { key: 'cpu_instructions', label: 'CPU Instructions', unit: 'instr', bold: true },
  { key: 'ram_bytes', label: 'Memory (RAM)', unit: 'bytes', bold: true },
  { key: 'ledger_read_bytes', label: 'Ledger Reads', unit: 'bytes', bold: false },
  { key: 'ledger_write_bytes', label: 'Ledger Writes', unit: 'bytes', bold: false },
  { key: 'transaction_size_bytes', label: 'Transaction Size', unit: 'bytes', bold: false },
]);

/** Coerce anything to a finite, non-negative number (defaults to 0). */
function toSafeNumber(value) {
  const num = typeof value === 'number' ? value : Number(value);
  if (!Number.isFinite(num) || num < 0) return 0;
  return num;
}

/**
 * Percent of the per-transaction limit consumed by `value`.
 *
 * Returns an unclamped percentage so callers can distinguish "at the limit"
 * (100) from "over budget" (>100); use `clampPercent` for bar widths.
 *
 * @param {number} value
 * @param {number} limit
 * @returns {number} percentage, or 0 when the limit is not a positive number
 */
function dailyValuePercent(value, limit) {
  const safeLimit = toSafeNumber(limit);
  if (safeLimit === 0) return 0;
  return (toSafeNumber(value) / safeLimit) * 100;
}

/** Clamp a percentage into the 0-100 range for progress-bar widths. */
function clampPercent(percent) {
  const num = toSafeNumber(percent);
  return num > 100 ? 100 : num;
}

/**
 * Severity bucket for a percent-of-limit reading.
 *
 * Thresholds follow the usual traffic-light convention so the label reads at
 * a glance: comfortable under half the budget, worth noticing past 75%, and
 * critical once the transaction is at or beyond the protocol ceiling.
 */
function severityFor(percent) {
  const num = toSafeNumber(percent);
  if (num >= 100) return 'over';
  if (num >= 75) return 'high';
  if (num >= 50) return 'moderate';
  return 'low';
}

/**
 * Build the full set of rendered metric rows.
 *
 * @param {Record<string, number>} resources per-transaction resource usage
 * @param {Record<string, number>} [limits] optional limit overrides
 */
function buildMetrics(resources, limits) {
  const effectiveLimits = { ...SOROBAN_LIMITS, ...(limits || {}) };
  const source = resources || {};

  return METRIC_DEFINITIONS.map((definition) => {
    const value = toSafeNumber(source[definition.key]);
    const limit = toSafeNumber(effectiveLimits[definition.key]);
    const percent = dailyValuePercent(value, limit);

    return {
      key: definition.key,
      label: definition.label,
      unit: definition.unit,
      bold: definition.bold,
      value,
      limit,
      percent,
      barPercent: clampPercent(percent),
      severity: severityFor(percent),
    };
  });
}

/**
 * Aggregate "calorie" figure for the label header.
 *
 * The single number a reader takes away is the worst-case resource pressure,
 * because a transaction fails when any one budget is exhausted -- not when
 * the average is high.
 */
function overallPercent(metrics) {
  if (!Array.isArray(metrics) || metrics.length === 0) return 0;
  return metrics.reduce((max, metric) => Math.max(max, toSafeNumber(metric.percent)), 0);
}

/** Format a raw resource count for display, using compact notation. */
function formatValue(value) {
  return new Intl.NumberFormat('en-US', {
    notation: 'compact',
    compactDisplay: 'short',
    maximumFractionDigits: 1,
  }).format(toSafeNumber(value));
}

/** Format a percentage for display, keeping sub-1% readings meaningful. */
function formatPercent(percent) {
  const num = toSafeNumber(percent);
  if (num > 0 && num < 0.1) return '<0.1%';
  return `${num.toFixed(1)}%`;
}

// ---------------------------------------------------------------------------
// Ledger footprint TTL & rent cost calculator
// ---------------------------------------------------------------------------

/** Number of ledgers closed per day (5s close time). */
const LEDGERS_PER_DAY = 17_280;

/** Stroops in one XLM (1 XLM = 10^7 stroops). */
const STROOPS_PER_XLM = 10_000_000;

/**
 * Selectable TTL durations for the rent calculator.
 *
 * `ledgers` is the number of ledgers the entry's TTL is extended by; it is
 * derived from the human-readable `days` value so the UI and the maths stay in
 * sync.
 */
const TTL_OPTIONS = Object.freeze([
  { id: '24h', label: '24h', days: 1, ledgers: 1 * LEDGERS_PER_DAY },
  { id: '7d', label: '7d', days: 7, ledgers: 7 * LEDGERS_PER_DAY },
  { id: '30d', label: '30d', days: 30, ledgers: 30 * LEDGERS_PER_DAY },
  { id: '90d', label: '90d', days: 90, ledgers: 90 * LEDGERS_PER_DAY },
  { id: '365d', label: '365d', days: 365, ledgers: 365 * LEDGERS_PER_DAY },
]);

/**
 * Rent rates per byte per ledger, in stroops.
 *
 * Persistent entries are charged the full rate; temporary entries are charged
 * a discounted rate. The values mirror the protocol's rent parameters and are
 * exported so deployments with custom rent settings can override them.
 */
const RENT_RATES = Object.freeze({
  persistent: 1.0,
  temporary: 0.5,
});

/** Default inclusion (base) fee in stroops. */
const DEFAULT_INCLUSION_FEE = 100;

/** Default resource execution fee in stroops. */
const DEFAULT_RESOURCE_FEE = 1_000;

/** Look up a TTL option by id, falling back to the first option. */
function resolveTtlOption(ttl) {
  if (ttl && typeof ttl === 'object' && Number.isFinite(Number(ttl.ledgers))) {
    return { id: ttl.id || 'custom', label: ttl.label || 'custom', ledgers: toSafeNumber(ttl.ledgers) };
  }
  const id = typeof ttl === 'string' ? ttl : ttl && ttl.id;
  const match = TTL_OPTIONS.find((option) => option.id === id);
  return match || TTL_OPTIONS[0];
}

/**
 * Projected rent for a single storage class (persistent or temporary).
 *
 * Rent is charged on the total footprint (reads + writes) for every ledger in
 * the TTL window, scaled by the class's per-byte rate.
 *
 * @param {number} footprintBytes total ledger bytes held by the entry
 * @param {number} ledgers TTL window in ledgers
 * @param {number} rate stroops per byte per ledger
 * @returns {number} rent in stroops
 */
function rentForClass(footprintBytes, ledgers, rate) {
  return toSafeNumber(footprintBytes) * toSafeNumber(ledgers) * toSafeNumber(rate);
}

/** Convert stroops to XLM. */
function stroopsToXlm(stroops) {
  return toSafeNumber(stroops) / STROOPS_PER_XLM;
}

/**
 * Compute the full fee breakdown for a TTL selection.
 *
 * @param {object} input
 * @param {number} [input.ledger_read_bytes]
 * @param {number} [input.ledger_write_bytes]
 * @param {string|object} [input.ttl] TTL option id (e.g. '30d') or option object
 * @param {number} [input.inclusionFee] inclusion fee in stroops
 * @param {number} [input.resourceFee] resource execution fee in stroops
 * @param {Record<string, number>} [input.rentRates] per-class rate overrides
 * @returns {object} breakdown with per-class rent, totals, and XLM values
 */
function computeRentBreakdown(input) {
  const source = input || {};
  const option = resolveTtlOption(source.ttl);
  const rates = { ...RENT_RATES, ...(source.rentRates || {}) };

  const footprintBytes =
    toSafeNumber(source.ledger_read_bytes) + toSafeNumber(source.ledger_write_bytes);

  const persistentStroops = rentForClass(footprintBytes, option.ledgers, rates.persistent);
  const temporaryStroops = rentForClass(footprintBytes, option.ledgers, rates.temporary);

  const inclusionFee = toSafeNumber(
    source.inclusionFee === undefined ? DEFAULT_INCLUSION_FEE : source.inclusionFee,
  );
  const resourceFee = toSafeNumber(
    source.resourceFee === undefined ? DEFAULT_RESOURCE_FEE : source.resourceFee,
  );

  // The TTL rent fee shown in the breakdown uses the persistent rate, since
  // persistent entries are the ones that must be kept alive indefinitely.
  const ttlRentFee = persistentStroops;
  const totalStroops = inclusionFee + resourceFee + ttlRentFee;

  return {
    ttl: option,
    footprintBytes,
    rates: { persistent: rates.persistent, temporary: rates.temporary },
    persistent: {
      stroops: persistentStroops,
      xlm: stroopsToXlm(persistentStroops),
    },
    temporary: {
      stroops: temporaryStroops,
      xlm: stroopsToXlm(temporaryStroops),
    },
    fees: {
      inclusionFee,
      resourceFee,
      ttlRentFee,
      totalStroops,
      totalXlm: stroopsToXlm(totalStroops),
    },
  };
}

/** Format an XLM amount for display with fixed precision. */
function formatXlm(xlm) {
  return `${toSafeNumber(xlm).toFixed(7)} XLM`;
}

module.exports = {
  SOROBAN_LIMITS,
  METRIC_DEFINITIONS,
  LEDGERS_PER_DAY,
  STROOPS_PER_XLM,
  TTL_OPTIONS,
  RENT_RATES,
  DEFAULT_INCLUSION_FEE,
  DEFAULT_RESOURCE_FEE,
  dailyValuePercent,
  clampPercent,
  severityFor,
  buildMetrics,
  overallPercent,
  formatValue,
  formatPercent,
  resolveTtlOption,
  rentForClass,
  stroopsToXlm,
  computeRentBreakdown,
  formatXlm,
};
