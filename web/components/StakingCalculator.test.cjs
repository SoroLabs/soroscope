const test = require('node:test');
const assert = require('node:assert/strict');

const { calculateStakingYield } = require('../lib/stakingCalculator');

test('StakingCalculator logic integration: default parameters yield calculation', () => {
  const result = calculateStakingYield();
  assert.equal(result.depositAmount, 1000);
  assert.equal(result.lockDurationMonths, 12);
  assert.equal(result.baseApyPercentage, 12);
  assert.equal(result.effectiveApyPercent, 18); // 12% * 1.5 multiplier
  assert.ok(result.totalBalance > 1000);
  assert.ok(result.totalInterest > 0);
});

test('StakingCalculator logic integration: max lock duration (36 months)', () => {
  const result = calculateStakingYield({
    depositAmount: 5000,
    lockDurationMonths: 36,
    baseApyPercentage: 10,
    compoundFrequency: 'daily',
    enableTierMultiplier: true,
  });

  // For 36 months, tier multiplier is 2.0x -> effective APY is 20%
  assert.equal(result.multiplier, 2.0);
  assert.equal(result.effectiveApyPercent, 20);
  assert.ok(result.totalBalance > 5000 * 1.6);
  assert.equal(result.breakdownByMonth.length, 36);
});
