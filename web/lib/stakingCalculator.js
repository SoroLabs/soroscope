/**
 * Staking & Yield Calculator Logic
 */

/**
 * Inclusive bounds for every numeric field rendered by the staking widget.
 * The UI inputs and the yield maths both read from this single source of
 * truth so a value can never be rendered/calculated outside a sane range.
 */
const STAKING_INPUT_LIMITS = {
  depositAmount: { min: 0, max: 1000000 },
  lockDurationMonths: { min: 1, max: 36 },
  baseApyPercentage: { min: 0, max: 100 },
};

const COMPOUND_FREQUENCIES = {
  daily: 365,
  weekly: 52,
  monthly: 12,
  quarterly: 4,
  annually: 1,
  none: 0,
};

const DURATION_TIER_MULTIPLIERS = {
  1: 1.0,
  3: 1.1,
  6: 1.25,
  12: 1.5,
  24: 1.75,
  36: 2.0,
};

/**
 * Get tier multiplier based on lock duration in months.
 * Interpolates or clamps appropriately.
 */
function getDurationTierMultiplier(months) {
  if (months <= 1) return 1.0;
  if (months <= 3) return 1.0 + (months - 1) * (0.1 / 2);
  if (months <= 6) return 1.1 + (months - 3) * (0.15 / 3);
  if (months <= 12) return 1.25 + (months - 6) * (0.25 / 6);
  if (months <= 24) return 1.5 + (months - 12) * (0.25 / 12);
  return 2.0;
}

/**
 * Coerce a raw numeric field value (usually a string straight out of an
 * `<input type="number">`) into a finite number clamped to `[min, max]`.
 *
 * Typing `-`, a lone `e`, pasting arbitrary text or overflowing the field
 * (e.g. `1e400`) makes the browser hand back a value that is not a usable
 * number: `Number()` yields `NaN`, and `Math.max(0, NaN)` is still `NaN`,
 * which used to leak `NaN`/`Infinity` into every projection in the widget.
 *
 * Resolution rules: blank/unparseable text collapses to `min` (0 for
 * deposits/APY), a value that overflows to `+Infinity` saturates at `max`, and
 * `-Infinity` saturates at `min`. The result is always a finite, in-range
 * number.
 */
function sanitizeNumericInput(rawValue, { min = 0, max = Number.MAX_SAFE_INTEGER } = {}) {
  const parsed = typeof rawValue === 'number' ? rawValue : Number(String(rawValue ?? '').trim());

  if (Number.isNaN(parsed)) return min;
  if (parsed === Infinity) return max;
  if (parsed === -Infinity) return min;

  return Math.min(Math.max(parsed, min), max);
}

/**
 * Calculate Staking Yield & APY
 */
function calculateStakingYield({
  depositAmount = 1000,
  lockDurationMonths = 12,
  compoundFrequency = 'monthly',
  baseApyPercentage = 12,
  enableTierMultiplier = true,
} = {}) {
  // Sanitize defensively as well as at the input boundary: callers passing
  // `NaN`, `Infinity` or negative values must not poison the results (nor spin
  // the monthly breakdown loop forever).
  const P = sanitizeNumericInput(depositAmount, STAKING_INPUT_LIMITS.depositAmount);
  const months = sanitizeNumericInput(lockDurationMonths, STAKING_INPUT_LIMITS.lockDurationMonths);
  const t = months / 12; // Time in years
  const baseApy = sanitizeNumericInput(baseApyPercentage, STAKING_INPUT_LIMITS.baseApyPercentage);

  const multiplier = enableTierMultiplier ? getDurationTierMultiplier(months) : 1.0;
  const effectiveApyPercent = baseApy * multiplier;
  const r = effectiveApyPercent / 100; // annual rate as decimal

  const n = COMPOUND_FREQUENCIES[compoundFrequency] ?? 12;

  let totalBalance = P;
  if (P === 0 || r === 0) {
    totalBalance = P;
  } else if (n === 0) {
    // Simple Interest: A = P * (1 + r * t)
    totalBalance = P * (1 + r * t);
  } else {
    // Compound Interest: A = P * (1 + r / n) ^ (n * t)
    totalBalance = P * Math.pow(1 + r / n, n * t);
  }

  const totalInterest = totalBalance - P;
  const totalRoiPercent = P > 0 ? (totalInterest / P) * 100 : 0;

  const totalDays = t * 365;
  const estimatedDailyYield = totalDays > 0 ? totalInterest / totalDays : 0;
  const estimatedMonthlyYield = months > 0 ? totalInterest / months : 0;

  // Monthly milestone projections
  const breakdownByMonth = [];
  for (let m = 1; m <= months; m++) {
    const elapsedYears = m / 12;
    let balanceAtM = P;
    if (P > 0 && r > 0) {
      if (n === 0) {
        balanceAtM = P * (1 + r * elapsedYears);
      } else {
        balanceAtM = P * Math.pow(1 + r / n, n * elapsedYears);
      }
    }
    const yieldAtM = balanceAtM - P;
    breakdownByMonth.push({
      month: m,
      balance: Number(balanceAtM.toFixed(2)),
      yieldEarned: Number(yieldAtM.toFixed(2)),
    });
  }

  return {
    depositAmount: P,
    lockDurationMonths: months,
    durationYears: Number(t.toFixed(4)),
    baseApyPercentage: baseApy,
    multiplier: Number(multiplier.toFixed(2)),
    effectiveApyPercent: Number(effectiveApyPercent.toFixed(2)),
    compoundFrequency,
    totalBalance: Number(totalBalance.toFixed(2)),
    totalInterest: Number(totalInterest.toFixed(2)),
    totalRoiPercent: Number(totalRoiPercent.toFixed(2)),
    estimatedDailyYield: Number(estimatedDailyYield.toFixed(4)),
    estimatedMonthlyYield: Number(estimatedMonthlyYield.toFixed(2)),
    breakdownByMonth,
  };
}

/** Query-string keys used to share a calculation as a link. */
const STAKING_QUERY_KEYS = ['deposit', 'apy', 'duration', 'compoundFrequency', 'tierMultiplier'];

function firstQueryValue(value) {
  return Array.isArray(value) ? value[0] : value;
}

/**
 * Parse shared-link query params into calculator state. Accepts a Next.js
 * `router.query` object or a plain `{ key: string }` map. Only keys that are
 * present are returned, and every value is clamped/validated, so a crafted
 * URL can never put the form into an invalid state.
 */
function parseStakingQuery(query = {}) {
  const parsed = {};

  const deposit = firstQueryValue(query.deposit);
  if (deposit !== undefined && deposit !== '') {
    parsed.depositAmount = sanitizeNumericInput(deposit, STAKING_INPUT_LIMITS.depositAmount);
  }

  const apy = firstQueryValue(query.apy);
  if (apy !== undefined && apy !== '') {
    parsed.baseApyPercentage = sanitizeNumericInput(apy, STAKING_INPUT_LIMITS.baseApyPercentage);
  }

  const duration = firstQueryValue(query.duration);
  if (duration !== undefined && duration !== '') {
    parsed.lockDurationMonths = Math.round(
      sanitizeNumericInput(duration, STAKING_INPUT_LIMITS.lockDurationMonths),
    );
  }

  const frequency = firstQueryValue(query.compoundFrequency);
  if (frequency !== undefined && Object.prototype.hasOwnProperty.call(COMPOUND_FREQUENCIES, frequency)) {
    parsed.compoundFrequency = frequency;
  }

  const tier = firstQueryValue(query.tierMultiplier);
  if (tier === 'true' || tier === 'false') {
    parsed.enableTierMultiplier = tier === 'true';
  }

  return parsed;
}

/** Serialize calculator state into shared-link query params (all strings). */
function buildStakingQuery({
  depositAmount,
  baseApyPercentage,
  lockDurationMonths,
  compoundFrequency,
  enableTierMultiplier,
}) {
  return {
    deposit: String(depositAmount),
    apy: String(baseApyPercentage),
    duration: String(lockDurationMonths),
    compoundFrequency: String(compoundFrequency),
    tierMultiplier: String(Boolean(enableTierMultiplier)),
  };
}

module.exports = {
  calculateStakingYield,
  getDurationTierMultiplier,
  sanitizeNumericInput,
  COMPOUND_FREQUENCIES,
  DURATION_TIER_MULTIPLIERS,
  STAKING_INPUT_LIMITS,
  STAKING_QUERY_KEYS,
  parseStakingQuery,
  buildStakingQuery,
};
