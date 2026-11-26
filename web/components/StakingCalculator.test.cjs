const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildStakingQuery,
  calculateStakingYield,
  parseStakingQuery,
  sanitizeNumericInput,
  STAKING_INPUT_LIMITS,
  STAKING_QUERY_KEYS,
} = require('../lib/stakingCalculator');

// The widget's number fields funnel raw typed text through
// `sanitizeNumericInput(value, STAKING_INPUT_LIMITS.<field>)`. These helpers
// reproduce that exact boundary so the tests exercise what `<input>` onChange
// does for hostile keystrokes instead of re-implementing the clamping rules.
const typeDeposit = (raw) =>
  sanitizeNumericInput(raw, STAKING_INPUT_LIMITS.depositAmount);
const typeBaseApy = (raw) =>
  sanitizeNumericInput(raw, STAKING_INPUT_LIMITS.baseApyPercentage);

test('StakingCalculator logic integration: default parameters yield calculation', () => {
  const result = calculateStakingYield();
  assert.equal(result.depositAmount, 1000);
  assert.equal(result.lockDurationMonths, 12);
  assert.equal(result.baseApyPercentage, 12);
  assert.equal(result.effectiveApyPercent, 18); // 12% * 1.5 multiplier
  assert.ok(result.totalBalance > 1000);
  assert.ok(result.totalInterest > 0);
});

test('StakingCalculator input boundary: negative deposit input falls back to 0', () => {
  assert.equal(typeDeposit('-500'), 0);
  assert.equal(typeDeposit('-'), 0);
  assert.equal(typeDeposit('-0'), 0);

  const results = calculateStakingYield({ depositAmount: typeDeposit('-500') });
  assert.equal(results.depositAmount, 0);
  assert.equal(results.totalBalance, 0);
  assert.equal(results.totalInterest, 0);
  assert.equal(results.totalRoiPercent, 0);
});

test('StakingCalculator input boundary: negative Base APY input falls back to 0', () => {
  assert.equal(typeBaseApy('-12'), 0);
  assert.equal(typeBaseApy('-'), 0);

  const results = calculateStakingYield({
    depositAmount: 1000,
    lockDurationMonths: 12,
    baseApyPercentage: typeBaseApy('-12'),
  });
  assert.equal(results.baseApyPercentage, 0);
  assert.equal(results.totalBalance, 1000);
  assert.equal(results.totalInterest, 0);
});

test('StakingCalculator input boundary: partial/blank text never yields NaN', () => {
  for (const raw of ['-', 'e', 'E', '', '   ', 'abc', 'NaN', undefined, null]) {
    const deposit = typeDeposit(raw);
    const apy = typeBaseApy(raw);

    assert.ok(Number.isFinite(deposit), `deposit "${String(raw)}" must be finite`);
    assert.ok(Number.isFinite(apy), `APY "${String(raw)}" must be finite`);
  }
});

test('StakingCalculator input boundary: overflowing input is clamped to the field max', () => {
  assert.equal(typeDeposit('1e400'), 1000000);
  assert.equal(typeDeposit(Infinity), 1000000);
  assert.equal(typeBaseApy('1e400'), 100);
  assert.equal(typeBaseApy(Infinity), 100);
});

test('StakingCalculator input boundary: valid typed values are preserved verbatim', () => {
  assert.equal(typeDeposit('7500'), 7500);
  assert.equal(typeDeposit('2500.5'), 2500.5);
  assert.equal(typeBaseApy('7.5'), 7.5);
  assert.equal(typeBaseApy('100'), 100);
  assert.equal(typeDeposit('1000000'), STAKING_INPUT_LIMITS.depositAmount.max);
});

test('StakingCalculator input boundary: hostile inputs render finite projection cards', () => {
  const results = calculateStakingYield({
    depositAmount: typeDeposit('-9999'),
    lockDurationMonths: 12,
    baseApyPercentage: typeBaseApy('not-a-number'),
  });

  const displayed = [
    results.totalBalance,
    results.totalInterest,
    results.totalRoiPercent,
    results.effectiveApyPercent,
    results.baseApyPercentage,
    results.estimatedDailyYield,
    results.estimatedMonthlyYield,
  ];

  for (const value of displayed) {
    assert.ok(Number.isFinite(value), `projected value ${value} must be finite`);
  }
  assert.ok(results.breakdownByMonth.every((row) => Number.isFinite(row.balance)));
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


test('StakingCalculator logic integration: compound frequency changes the projected return', () => {
  const base = {
    depositAmount: 10000,
    lockDurationMonths: 12,
    baseApyPercentage: 10,
    enableTierMultiplier: false,
  };

  const simple = calculateStakingYield({ ...base, compoundFrequency: 'none' });
  const monthly = calculateStakingYield({ ...base, compoundFrequency: 'monthly' });
  const daily = calculateStakingYield({ ...base, compoundFrequency: 'daily' });

  assert.equal(simple.totalBalance, 11000);
  assert.ok(monthly.totalBalance > simple.totalBalance);
  assert.ok(daily.totalBalance >= monthly.totalBalance);
});

test('StakingCalculator logic integration: milestone table contains monthly projections', () => {
  const result = calculateStakingYield({
    depositAmount: 2500,
    lockDurationMonths: 6,
    baseApyPercentage: 8,
    compoundFrequency: 'weekly',
  });

  assert.equal(result.breakdownByMonth.length, 6);
  assert.equal(result.breakdownByMonth[0].month, 1);
  assert.equal(result.breakdownByMonth.at(-1).month, 6);
  assert.ok(result.breakdownByMonth.at(-1).balance > result.breakdownByMonth[0].balance);
});

test('StakingCalculator share link: query round-trips every parameter', () => {
  const state = {
    depositAmount: 2500,
    baseApyPercentage: 8.5,
    lockDurationMonths: 24,
    compoundFrequency: 'weekly',
    enableTierMultiplier: false,
  };
  const query = buildStakingQuery(state);
  assert.deepEqual(Object.keys(query).sort(), [...STAKING_QUERY_KEYS].sort());
  assert.deepEqual(query, {
    deposit: '2500',
    apy: '8.5',
    duration: '24',
    compoundFrequency: 'weekly',
    tierMultiplier: 'false',
  });
  assert.deepEqual(parseStakingQuery(query), state);
});

test('StakingCalculator share link: missing params are left out', () => {
  assert.deepEqual(parseStakingQuery({}), {});
  assert.deepEqual(parseStakingQuery({ apy: '5', unrelated: 'x' }), { baseApyPercentage: 5 });
});

test('StakingCalculator share link: hostile values are clamped or ignored', () => {
  const parsed = parseStakingQuery({
    deposit: '-100',
    apy: '9999',
    duration: '99.6',
    compoundFrequency: '__proto__',
    tierMultiplier: 'yes',
  });
  assert.equal(parsed.depositAmount, STAKING_INPUT_LIMITS.depositAmount.min);
  assert.equal(parsed.baseApyPercentage, STAKING_INPUT_LIMITS.baseApyPercentage.max);
  assert.equal(parsed.lockDurationMonths, STAKING_INPUT_LIMITS.lockDurationMonths.max);
  assert.equal(parsed.compoundFrequency, undefined);
  assert.equal(parsed.enableTierMultiplier, undefined);

  assert.equal(parseStakingQuery({ deposit: 'abc' }).depositAmount, 0);
  assert.equal(parseStakingQuery({ duration: '6.4' }).lockDurationMonths, 6);
});

test('StakingCalculator share link: repeated params use the first value', () => {
  assert.deepEqual(parseStakingQuery({ deposit: ['300', '900'] }), { depositAmount: 300 });
});
