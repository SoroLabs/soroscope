const test = require('node:test');
const assert = require('node:assert/strict');

const {
  calculateStakingYield,
  getDurationTierMultiplier,
  COMPOUND_FREQUENCIES,
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

