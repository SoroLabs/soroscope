// FeeEstimationPreview.test.cjs — unit tests for FeeEstimationPreview state and bump selection logic
// Closes Issue #595
// Runs with: node --test ./components/FeeEstimationPreview.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');


// Exercise the production helper instead of a separate copy of its calculations.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const ts = require('typescript');
const sourcePath = path.join(__dirname, '../lib/stellarRpc.ts');
const { outputText } = ts.transpileModule(fs.readFileSync(sourcePath, 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 },
});
const production = { exports: {} };
vm.runInNewContext(outputText, {
  exports: production.exports,
  AbortController,
  setTimeout,
  clearTimeout,
  fetch: (...args) => globalThis.fetch(...args),
}, { filename: sourcePath });
const { estimateFees, fetchFeeStats } = production.exports;

for (const baseFee of [0, undefined]) {
  test(`FeeEstimationPreview: defaults ${baseFee} base fee to 100 stroops`, () => {
    const estimate = estimateFees(500_000, {
      min_ledger_fee: baseFee, surge_multiplier: 1, soroban_fee_rate: 1,
    });
    assert.equal(estimate.classicFeeStroops, 100);
    assert.equal(estimate.totalFeeStroops, 500_100);
    assert.equal(estimate.totalFeeXlm, '0.05001');
    assert.equal(estimate.surgeMultiplier, 1);
    assert.deepEqual(Array.from(estimate.feeBumps, bump => bump.feeStroops), [500_100, 750_150, 1_000_200]);
    for (const bump of estimate.feeBumps) {
      assert.ok(Number.isFinite(bump.feeStroops));
      assert.ok(Number.isFinite(Number(bump.feeXlm)));
    }
  });
}

test('FeeEstimationPreview: preserves a nonzero network base fee', () => {
  const estimate = estimateFees(500_000, { min_ledger_fee: 250, surge_multiplier: 2, soroban_fee_rate: 1 });
  assert.equal(estimate.classicFeeStroops, 250);
  assert.equal(estimate.totalFeeStroops, 500_250);
  assert.equal(estimate.surgeMultiplier, 2);
});

test('FeeEstimationPreview: zero RPC ledger fee uses the fallback in the estimate', async (t) => {
  t.mock.method(globalThis, 'fetch', async () => ({
    ok: true,
    json: async () => ({ result: { last_ledger_base_fee: '0', surge_multiplier: '1', soroban_fee_rate: '1' } }),
  }));
  const stats = await fetchFeeStats('https://rpc.example');
  const estimate = estimateFees(500_000, stats);
  assert.equal(estimate.classicFeeStroops, 100);
  assert.equal(estimate.totalFeeXlm, '0.05001');
});

function createFeeEstimationState(options = {}) {
  const {
    costStroops = 500_000,
    rpcAvailable = true,
  } = options;

  let feeEstimate = null;
  let selectedBump = 'Low (Min)';
  let fetching = false;
  let isRpcAvailable = rpcAvailable;
  let lastCallbackArgs = null;

  function loadFeeEstimate(stats) {
    if (costStroops <= 0) {
      feeEstimate = null;
      return;
    }
    fetching = true;
    // Simulate real component: if stats is null/undefined,
    // RPC fetch failed (fetchFeeStats returned null or threw).
    if (stats) {
      feeEstimate = estimateFees(costStroops, stats);
      isRpcAvailable = true;
    } else {
      feeEstimate = estimateFees(costStroops, null);
      isRpcAvailable = false;
    }
    fetching = false;
  }

  function handleBumpSelect(bump, callback) {
    selectedBump = bump.label;
    if (callback) {
      const level = bump.label.toLowerCase().includes('low') ? 'low'
        : bump.label.toLowerCase().includes('high') ? 'high'
        : 'medium';
      lastCallbackArgs = { level, feeStroops: bump.feeStroops };
      callback(level, bump.feeStroops);
    }
  }

  function refresh() {
    loadFeeEstimate(isRpcAvailable ? { min_ledger_fee: 100, surge_multiplier: 1, soroban_fee_rate: 1 } : null);
  }

  return {
    get feeEstimate() { return feeEstimate; },
    get selectedBump() { return selectedBump; },
    get isFetching() { return fetching; },
    get isRpcAvailable() { return isRpcAvailable; },
    get lastCallbackArgs() { return lastCallbackArgs; },
    loadFeeEstimate,
    handleBumpSelect,
    refresh,
    setRpcAvailable(v) { isRpcAvailable = v; },
    setSelectedBump(v) { selectedBump = v; },
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

test('FeeEstimationPreview: initial state is empty when no cost data', () => {
  const state = createFeeEstimationState({ costStroops: 0 });
  state.loadFeeEstimate(null);
  assert.equal(state.feeEstimate, null);
});

test('FeeEstimationPreview: loads fee estimate from resource cost', () => {
  const state = createFeeEstimationState({ costStroops: 500_000 });
  const mockStats = { min_ledger_fee: 100, surge_multiplier: 1, soroban_fee_rate: 1 };
  state.loadFeeEstimate(mockStats);

  assert.ok(state.feeEstimate !== null);
  assert.equal(state.feeEstimate.minResourceFeeStroops, 500_000);
  assert.equal(state.feeEstimate.classicFeeStroops, 100);
  assert.ok(state.feeEstimate.feeBumps.length === 3);
});

test('FeeEstimationPreview: shows three fee bump options', () => {
  const state = createFeeEstimationState();
  state.loadFeeEstimate(null);

  const bumps = state.feeEstimate.feeBumps;
  assert.equal(bumps.length, 3);

  assert.equal(bumps[0].label, 'Low (Min)');
  assert.equal(bumps[1].label, 'Medium');
  assert.equal(bumps[2].label, 'High');
});

test('FeeEstimationPreview: selecting a fee bump updates selected state', () => {
  const state = createFeeEstimationState();
  state.loadFeeEstimate(null);

  const bumps = state.feeEstimate.feeBumps;
  state.handleBumpSelect(bumps[1]);

  assert.equal(state.selectedBump, 'Medium');
});

test('FeeEstimationPreview: fee bump selection fires callback with correct level', () => {
  const state = createFeeEstimationState();
  state.loadFeeEstimate(null);

  const bumps = state.feeEstimate.feeBumps;
  let callbackResult = null;

  state.handleBumpSelect(bumps[2], (level, feeStroops) => {
    callbackResult = { level, feeStroops };
  });

  assert.equal(callbackResult.level, 'high');
  assert.ok(callbackResult.feeStroops > 0);
});

test('FeeEstimationPreview: low bump has lowest fee, high has highest', () => {
  const state = createFeeEstimationState();
  state.loadFeeEstimate(null);

  const fees = state.feeEstimate.networkFees;
  assert.ok(fees.low < fees.medium, 'Low < Medium');
  assert.ok(fees.medium < fees.high, 'Medium < High');
});

test('FeeEstimationPreview: rpc offline flag shown when stats fetch fails', () => {
  const state = createFeeEstimationState({ rpcAvailable: false });
  state.loadFeeEstimate(null);

  assert.equal(state.isRpcAvailable, false);
  assert.ok(state.feeEstimate !== null);
});

test('FeeEstimationPreview: refresh reloads fee estimate', () => {
  const state = createFeeEstimationState({ costStroops: 100_000 });
  state.loadFeeEstimate(null);

  const beforeFee = state.feeEstimate.totalFeeStroops;

  state.setRpcAvailable(true);
  state.refresh();

  assert.ok(state.feeEstimate !== null);
  assert.equal(state.feeEstimate.totalFeeStroops, beforeFee);
});

test('FeeEstimationPreview: returns correct fee breakdown for a known resource cost', () => {
  const state = createFeeEstimationState({ costStroops: 1_234_567 });
  state.loadFeeEstimate({ min_ledger_fee: 100, surge_multiplier: 1, soroban_fee_rate: 1 });

  const est = state.feeEstimate;
  assert.equal(est.minResourceFeeStroops, 1_234_567);
  assert.equal(est.classicFeeStroops, 100);
  assert.equal(est.totalFeeStroops, 1_234_667);
  assert.equal(est.surgeMultiplier, 1);
});

test('FeeEstimationPreview: surge multiplier > 1 increases all fee bump levels', () => {
  const baseCost = 500_000;
  const normal = estimateFees(baseCost, { min_ledger_fee: 100, surge_multiplier: 1, soroban_fee_rate: 1 });
  const surged = estimateFees(baseCost, { min_ledger_fee: 100, surge_multiplier: 3, soroban_fee_rate: 2 });

  assert.ok(surged.surgeMultiplier > normal.surgeMultiplier);
  assert.ok(surged.networkFees.low > normal.networkFees.low);
  assert.ok(surged.networkFees.medium > normal.networkFees.medium);
  assert.ok(surged.networkFees.high > normal.networkFees.high);
});

test('FeeEstimationPreview: handles very small resource costs', () => {
  const state = createFeeEstimationState({ costStroops: 1 });
  state.loadFeeEstimate(null);

  assert.ok(state.feeEstimate !== null);
  assert.equal(state.feeEstimate.minResourceFeeStroops, 1);
  assert.ok(state.feeEstimate.totalFeeStroops > 1);
});

test('FeeEstimationPreview: defaults base fee to 100 stroops when base_fee is zero', () => {
  const state = createFeeEstimationState({ costStroops: 500_000 });
  state.loadFeeEstimate({ min_ledger_fee: 0, surge_multiplier: 1, soroban_fee_rate: 1 });

  const est = state.feeEstimate;
  assert.equal(est.classicFeeStroops, 100);
  assert.equal(est.totalFeeStroops, 500_100);
  assert.ok(!Number.isNaN(est.totalFeeStroops));
  assert.ok(!est.totalFeeXlm.includes('NaN'));
  for (const bump of est.feeBumps) {
    assert.ok(!Number.isNaN(bump.feeStroops));
    assert.ok(!bump.feeXlm.includes('NaN'));
  }
});

test('FeeEstimationPreview: handles very large resource costs without overflow', () => {
  const state = createFeeEstimationState({ costStroops: 1_000_000_000_000 });
  state.loadFeeEstimate(null);

  assert.ok(state.feeEstimate !== null);
  assert.ok(state.feeEstimate.totalFeeStroops > 1_000_000_000_000);
  assert.ok(Number.isFinite(state.feeEstimate.totalFeeStroops));
});
