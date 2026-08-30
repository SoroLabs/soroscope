/**
 * Staking & Yield Calculator Logic
 */

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
 * Calculate Staking Yield & APY
 */
function calculateStakingYield({
  depositAmount = 1000,
  lockDurationMonths = 12,
  compoundFrequency = 'monthly',
  baseApyPercentage = 12,
  enableTierMultiplier = true,
} = {}) {
  const P = Math.max(0, Number(depositAmount) || 0);
  const months = Math.max(1, Number(lockDurationMonths) || 1);
  const t = months / 12; // Time in years
  const baseApy = Math.max(0, Number(baseApyPercentage) || 0);

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

module.exports = {
  calculateStakingYield,
  getDurationTierMultiplier,
  COMPOUND_FREQUENCIES,
  DURATION_TIER_MULTIPLIERS,
};
