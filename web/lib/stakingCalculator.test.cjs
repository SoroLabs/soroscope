const test = require('node:test');
const assert = require('node:assert/strict');

const {
  calculateStakingYield,
  getDurationTierMultiplier,
  sanitizeNumericInput,
  COMPOUND_FREQUENCIES,
  STAKING_INPUT_LIMITS,
} = require('./stakingCalculator');

test('calculateStakingYield: zero deposit returns 0 interest and 0 ROI', () => {
  const result = calculateStakingYield({ depositAmount: 0 });
  assert.equal(result.totalBalance, 0);
  assert.equal(result.totalInterest, 0);
  assert.equal(result.totalRoiPercent, 0);
});

test('calculateStakingYield: 0% APY returns deposit amount as total balance', () => {
  const result = calculateStakingYield({ depositAmount: 1000, baseApyPercentage: 0 });
  assert.equal(result.totalBalance, 1000);
  assert.equal(result.totalInterest, 0);
});

test('calculateStakingYield: simple interest (none compounding)', () => {
  const result = calculateStakingYield({
    depositAmount: 1000,
    lockDurationMonths: 12,
    baseApyPercentage: 10,
    compoundFrequency: 'none',
    enableTierMultiplier: false,
  });

  // A = 1000 * (1 + 0.10 * 1) = 1100
  assert.equal(result.totalBalance, 1100);
  assert.equal(result.totalInterest, 100);
  assert.equal(result.totalRoiPercent, 10);
});

test('calculateStakingYield: compound monthly interest', () => {
  const result = calculateStakingYield({
    depositAmount: 1000,
    lockDurationMonths: 12,
    baseApyPercentage: 12,
    compoundFrequency: 'monthly',
    enableTierMultiplier: false,
  });

  // A = 1000 * (1 + 0.12 / 12) ^ 12 = 1000 * (1.01)^12 ≈ 1126.83
  assert.equal(result.totalBalance, 1126.83);
  assert.equal(result.totalInterest, 126.83);
  assert.equal(result.totalRoiPercent, 12.68);
  assert.equal(result.breakdownByMonth.length, 12);
});

test('getDurationTierMultiplier: duration scaling thresholds', () => {
  assert.equal(getDurationTierMultiplier(1), 1.0);
  assert.equal(getDurationTierMultiplier(3), 1.1);
  assert.equal(getDurationTierMultiplier(6), 1.25);
  assert.equal(getDurationTierMultiplier(12), 1.5);
  assert.equal(getDurationTierMultiplier(24), 1.75);
  assert.equal(getDurationTierMultiplier(36), 2.0);
});

test('calculateStakingYield: tier multiplier increases effective APY', () => {
  const withMultiplier = calculateStakingYield({
    depositAmount: 1000,
    lockDurationMonths: 12,
    baseApyPercentage: 10,
    enableTierMultiplier: true,
  });

  // For 12 months, tier multiplier is 1.5x, so effective APY = 15%
  assert.equal(withMultiplier.multiplier, 1.5);
  assert.equal(withMultiplier.effectiveApyPercent, 15);
});

test('calculateStakingYield: compound frequencies affect total interest appropriately', () => {
  const daily = calculateStakingYield({ depositAmount: 1000, compoundFrequency: 'daily', enableTierMultiplier: false });
  const monthly = calculateStakingYield({ depositAmount: 1000, compoundFrequency: 'monthly', enableTierMultiplier: false });
  const annually = calculateStakingYield({ depositAmount: 1000, compoundFrequency: 'annually', enableTierMultiplier: false });
  const simple = calculateStakingYield({ depositAmount: 1000, compoundFrequency: 'none', enableTierMultiplier: false });

  // Higher frequency compounding yields strictly higher or equal returns: Daily >= Monthly >= Annually >= Simple
  assert.ok(daily.totalInterest >= monthly.totalInterest);
  assert.ok(monthly.totalInterest >= annually.totalInterest);
  assert.ok(annually.totalInterest >= simple.totalInterest);
});

test('calculateStakingYield: handles negative inputs and boundary clamping', () => {
  const negativeDeposit = calculateStakingYield({ depositAmount: -500, lockDurationMonths: -5 });
  assert.equal(negativeDeposit.depositAmount, 0);
  assert.equal(negativeDeposit.lockDurationMonths, 1);
  assert.equal(negativeDeposit.totalBalance, 0);
});

test('sanitizeNumericInput: negative, blank and unparseable text collapse to the minimum', () => {
  // Regression coverage for the negative-input boundary bug (issue #137):
  // `Math.max(0, Number(''))` is fine, but `Math.max(0, Number('abc'))` is NaN,
  // so the field boundary must filter non-finite values instead of clamping them.
  const deposit = STAKING_INPUT_LIMITS.depositAmount;

  assert.equal(sanitizeNumericInput('-500', deposit), 0);
  assert.equal(sanitizeNumericInput(-1, deposit), 0);
  assert.equal(sanitizeNumericInput('-0.0001', deposit), 0);
  assert.equal(sanitizeNumericInput('-', deposit), 0);
  assert.equal(sanitizeNumericInput('e', deposit), 0);
  assert.equal(sanitizeNumericInput('', deposit), 0);
  assert.equal(sanitizeNumericInput('   ', deposit), 0);
  assert.equal(sanitizeNumericInput('abc', deposit), 0);
  assert.equal(sanitizeNumericInput(null, deposit), 0);
  assert.equal(sanitizeNumericInput(undefined, deposit), 0);
  assert.equal(sanitizeNumericInput(NaN, deposit), 0);

  for (const raw of ['-500', '-', 'e', '', 'abc', NaN, null, undefined, -Infinity]) {
    const value = sanitizeNumericInput(raw, deposit);
    assert.ok(Number.isFinite(value), `${String(raw)} must sanitize to a finite number`);
    assert.equal(value, 0, `${String(raw)} must fall back to 0`);
  }
});

test('sanitizeNumericInput: clamps values above the field maximum', () => {
  const deposit = STAKING_INPUT_LIMITS.depositAmount;
  const apy = STAKING_INPUT_LIMITS.baseApyPercentage;
  const months = STAKING_INPUT_LIMITS.lockDurationMonths;

  assert.equal(sanitizeNumericInput('1e400', deposit), deposit.max);
  assert.equal(sanitizeNumericInput(Infinity, deposit), deposit.max);
  assert.equal(sanitizeNumericInput('9999999999', deposit), deposit.max);
  assert.equal(sanitizeNumericInput('150', apy), 100);
  assert.equal(sanitizeNumericInput('120', months), 36);
  assert.equal(sanitizeNumericInput('0', months), 1);
});

test('sanitizeNumericInput: preserves valid decimal input inside the range', () => {
  const deposit = STAKING_INPUT_LIMITS.depositAmount;
  const apy = STAKING_INPUT_LIMITS.baseApyPercentage;

  assert.equal(sanitizeNumericInput('2500.75', deposit), 2500.75);
  assert.equal(sanitizeNumericInput('12.5', apy), 12.5);
  assert.equal(sanitizeNumericInput(' 42 ', apy), 42);
  assert.equal(sanitizeNumericInput(0, deposit), 0);
  assert.equal(sanitizeNumericInput(1000000, deposit), 1000000);
});

test('calculateStakingYield: non-finite inputs never leak NaN or Infinity into projections', () => {
  const nan = calculateStakingYield({
    depositAmount: NaN,
    lockDurationMonths: NaN,
    baseApyPercentage: NaN,
  });

  assert.equal(nan.depositAmount, 0);
  assert.equal(nan.lockDurationMonths, 1);
  assert.equal(nan.baseApyPercentage, 0);
  assert.equal(nan.totalBalance, 0);
  assert.equal(nan.totalInterest, 0);
  assert.equal(nan.totalRoiPercent, 0);

  const infinite = calculateStakingYield({
    depositAmount: Infinity,
    lockDurationMonths: Infinity,
    baseApyPercentage: Infinity,
  });

  for (const [key, value] of Object.entries(infinite)) {
    if (typeof value === 'number') {
      assert.ok(Number.isFinite(value), `result.${key} must stay finite, received ${value}`);
    }
  }
  assert.ok(infinite.breakdownByMonth.every((row) => Number.isFinite(row.balance)));
  assert.ok(infinite.breakdownByMonth.every((row) => Number.isFinite(row.yieldEarned)));
});

test('calculateStakingYield: monthly breakdown matches final balance', () => {
  const result = calculateStakingYield({
    depositAmount: 2000,
    lockDurationMonths: 24,
    baseApyPercentage: 8,
    compoundFrequency: 'monthly',
    enableTierMultiplier: true,
  });

  assert.equal(result.breakdownByMonth.length, 24);
  const lastMonth = result.breakdownByMonth[result.breakdownByMonth.length - 1];
  assert.equal(lastMonth.month, 24);
  assert.equal(lastMonth.balance, result.totalBalance);
  assert.equal(lastMonth.yieldEarned, result.totalInterest);
});

