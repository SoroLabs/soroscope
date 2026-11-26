// nutritionLabel.test.cjs — unit tests for the nutrition label calculations
// Closes Issue #44
// Runs with: node --test ./lib/nutritionLabel.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  SOROBAN_LIMITS,
  METRIC_DEFINITIONS,
  dailyValuePercent,
  clampPercent,
  severityFor,
  buildMetrics,
  overallPercent,
  formatValue,
  formatPercent,
} = require('./nutritionLabel.js');

// ── dailyValuePercent ────────────────────────────────────────────────────────

test('dailyValuePercent computes a straight percentage of the limit', () => {
  assert.equal(dailyValuePercent(50, 100), 50);
  assert.equal(dailyValuePercent(25, 200), 12.5);
  assert.equal(dailyValuePercent(0, 100), 0);
});

test('dailyValuePercent reports overruns above 100 rather than saturating', () => {
  assert.equal(dailyValuePercent(150, 100), 150);
  assert.equal(dailyValuePercent(300, 100), 300);
});

test('dailyValuePercent returns 0 when the limit is missing or non-positive', () => {
  assert.equal(dailyValuePercent(50, 0), 0);
  assert.equal(dailyValuePercent(50, -10), 0);
  assert.equal(dailyValuePercent(50, undefined), 0);
  assert.equal(dailyValuePercent(50, NaN), 0);
});

test('dailyValuePercent coerces invalid usage values to zero', () => {
  assert.equal(dailyValuePercent(undefined, 100), 0);
  assert.equal(dailyValuePercent(null, 100), 0);
  assert.equal(dailyValuePercent(NaN, 100), 0);
  assert.equal(dailyValuePercent(-5, 100), 0);
  assert.equal(dailyValuePercent(Infinity, 100), 0);
});

test('dailyValuePercent accepts numeric strings', () => {
  assert.equal(dailyValuePercent('50', 100), 50);
});

// ── clampPercent ─────────────────────────────────────────────────────────────

test('clampPercent caps values at 100 but leaves smaller ones intact', () => {
  assert.equal(clampPercent(42.5), 42.5);
  assert.equal(clampPercent(100), 100);
  assert.equal(clampPercent(250), 100);
  assert.equal(clampPercent(-5), 0);
});

// ── severityFor ──────────────────────────────────────────────────────────────

test('severityFor buckets percentages into traffic-light bands', () => {
  assert.equal(severityFor(0), 'low');
  assert.equal(severityFor(49.9), 'low');
  assert.equal(severityFor(50), 'moderate');
  assert.equal(severityFor(74.9), 'moderate');
  assert.equal(severityFor(75), 'high');
  assert.equal(severityFor(99.9), 'high');
  assert.equal(severityFor(100), 'over');
  assert.equal(severityFor(500), 'over');
});

// ── buildMetrics ─────────────────────────────────────────────────────────────

const SAMPLE = {
  cpu_instructions: 50_000_000,
  ram_bytes: 20 * 1024 * 1024,
  ledger_read_bytes: 32 * 1024,
  ledger_write_bytes: 16 * 1024,
  transaction_size_bytes: 64 * 1024,
};

test('buildMetrics returns one row per metric definition, in order', () => {
  const metrics = buildMetrics(SAMPLE);
  assert.equal(metrics.length, METRIC_DEFINITIONS.length);
  assert.deepEqual(
    metrics.map((m) => m.key),
    METRIC_DEFINITIONS.map((d) => d.key),
  );
});

test('buildMetrics computes percentages against the Soroban limits', () => {
  const metrics = buildMetrics(SAMPLE);
  const byKey = Object.fromEntries(metrics.map((m) => [m.key, m]));

  // 50M of the 100M instruction budget.
  assert.equal(byKey.cpu_instructions.percent, 50);
  // 20MiB of the 40MiB memory budget.
  assert.equal(byKey.ram_bytes.percent, 50);
  // 32KiB of the 64KiB read budget.
  assert.equal(byKey.ledger_read_bytes.percent, 50);
  // 16KiB of the 64KiB write budget.
  assert.equal(byKey.ledger_write_bytes.percent, 25);
  // 64KiB of the 128KiB transaction size budget.
  assert.equal(byKey.transaction_size_bytes.percent, 50);
});

test('buildMetrics carries the limit and severity onto each row', () => {
  const metrics = buildMetrics(SAMPLE);
  const cpu = metrics.find((m) => m.key === 'cpu_instructions');
  assert.equal(cpu.limit, SOROBAN_LIMITS.cpu_instructions);
  assert.equal(cpu.severity, 'moderate');
  assert.equal(cpu.label, 'CPU Instructions');
  assert.equal(cpu.unit, 'instr');
});

test('buildMetrics clamps the bar width but keeps the true percentage', () => {
  const metrics = buildMetrics({ ...SAMPLE, cpu_instructions: 250_000_000 });
  const cpu = metrics.find((m) => m.key === 'cpu_instructions');
  assert.equal(cpu.percent, 250);
  assert.equal(cpu.barPercent, 100);
  assert.equal(cpu.severity, 'over');
});

test('buildMetrics tolerates missing, partial and undefined input', () => {
  for (const input of [undefined, null, {}, { cpu_instructions: 1_000_000 }]) {
    const metrics = buildMetrics(input);
    assert.equal(metrics.length, METRIC_DEFINITIONS.length);
    for (const metric of metrics) {
      assert.ok(Number.isFinite(metric.percent), 'percent must stay finite');
      assert.ok(metric.value >= 0, 'value must never be negative');
    }
  }
});

test('buildMetrics honours per-deployment limit overrides', () => {
  const metrics = buildMetrics(
    { cpu_instructions: 50_000_000 },
    { cpu_instructions: 50_000_000 },
  );
  const cpu = metrics.find((m) => m.key === 'cpu_instructions');
  assert.equal(cpu.limit, 50_000_000);
  assert.equal(cpu.percent, 100);
  assert.equal(cpu.severity, 'over');
});

test('buildMetrics leaves the shared limit table unmutated', () => {
  const before = { ...SOROBAN_LIMITS };
  buildMetrics(SAMPLE, { cpu_instructions: 1 });
  assert.deepEqual({ ...SOROBAN_LIMITS }, before);
});

// ── overallPercent ───────────────────────────────────────────────────────────

test('overallPercent reports the worst metric, not the average', () => {
  const metrics = buildMetrics({
    ...SAMPLE,
    cpu_instructions: 90_000_000, // 90%
    ledger_write_bytes: 1024, // ~1.6%
  });
  assert.equal(overallPercent(metrics), 90);
});

test('overallPercent is 0 for an empty or invalid metric list', () => {
  assert.equal(overallPercent([]), 0);
  assert.equal(overallPercent(undefined), 0);
  assert.equal(overallPercent(null), 0);
});

// ── formatting ───────────────────────────────────────────────────────────────

test('formatValue renders large counts in compact notation', () => {
  assert.equal(formatValue(0), '0');
  assert.equal(formatValue(999), '999');
  assert.equal(formatValue(1_500), '1.5K');
  assert.equal(formatValue(50_000_000), '50M');
});

test('formatValue coerces invalid input to zero rather than throwing', () => {
  assert.equal(formatValue(undefined), '0');
  assert.equal(formatValue(NaN), '0');
  assert.equal(formatValue(-1), '0');
});

test('formatPercent keeps one decimal and flags sub-0.1% readings', () => {
  assert.equal(formatPercent(0), '0.0%');
  assert.equal(formatPercent(12.34), '12.3%');
  assert.equal(formatPercent(100), '100.0%');
  assert.equal(formatPercent(0.05), '<0.1%');
});
