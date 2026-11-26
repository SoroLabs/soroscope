// resource-label.test.cjs — unit tests for headroom warning gauges & protocol limit logic
// Closes Issue #1454
// Runs with: node --test ./components/resource-label.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const DEFAULT_SOROBAN_LIMITS = {
  cpuInstructions: 100_000_000,
  ramBytes: 40 * 1024 * 1024,
  writeBytes: 64 * 1024,
  footprintKeys: 100,
};

function calculateHeadroom(used, limit) {
  if (limit <= 0) {
    return { consumedPct: 100, headroomPct: 0, txsPerLedger: 0, isExceeded: true };
  }
  const consumedPct = Math.round((used / limit) * 1000) / 10;
  const headroomPct = Math.max(0, Math.round((100 - (used / limit) * 100) * 10) / 10);
  const txsPerLedger = used > 0 ? Math.floor(limit / used) : Infinity;
  const isExceeded = used > limit || consumedPct > 100;
  return { consumedPct, headroomPct, txsPerLedger, isExceeded };
}

function getStatusColor(value) {
  if (value > 85) return 'critical-pulsing-red';
  if (value >= 60) return 'warning-amber';
  return 'safe-green';
}

function checkSystemStatus(metrics, limits = DEFAULT_SOROBAN_LIMITS) {
  const isAnyExceeded =
    metrics.cpuUsage > 100 ||
    metrics.ramUsage > 100 ||
    metrics.footprintUsage > 100 ||
    (metrics.writeBytesUsage !== undefined && metrics.writeBytesUsage > 100) ||
    (metrics.rawCpu !== undefined && metrics.rawCpu > limits.cpuInstructions) ||
    (metrics.rawRam !== undefined && metrics.rawRam > limits.ramBytes) ||
    (metrics.rawFootprint !== undefined && metrics.rawFootprint > limits.footprintKeys) ||
    (metrics.rawWriteBytes !== undefined && metrics.rawWriteBytes > limits.writeBytes);

  const hasCritical =
    metrics.cpuUsage > 85 ||
    metrics.ramUsage > 85 ||
    metrics.footprintUsage > 85 ||
    (metrics.writeBytesUsage ?? 0) > 85 ||
    isAnyExceeded;

  if (isAnyExceeded) return 'Exceeds Network Limit';
  if (hasCritical) return 'Critical';
  return 'Active';
}

test('calculateHeadroom computes correct percentages and headroom for CPU', () => {
  const limit = DEFAULT_SOROBAN_LIMITS.cpuInstructions; // 100M
  const res = calculateHeadroom(25_000_000, limit);
  assert.equal(res.consumedPct, 25);
  assert.equal(res.headroomPct, 75);
  assert.equal(res.txsPerLedger, 4);
  assert.equal(res.isExceeded, false);
});

test('calculateHeadroom handles full usage and zero headroom', () => {
  const limit = DEFAULT_SOROBAN_LIMITS.footprintKeys; // 100
  const res = calculateHeadroom(100, limit);
  assert.equal(res.consumedPct, 100);
  assert.equal(res.headroomPct, 0);
  assert.equal(res.txsPerLedger, 1);
  assert.equal(res.isExceeded, false);
});

test('calculateHeadroom detects exceeded limit', () => {
  const limit = DEFAULT_SOROBAN_LIMITS.writeBytes; // 64KB
  const res = calculateHeadroom(70_000, limit);
  assert.equal(res.isExceeded, true);
  assert.equal(res.headroomPct, 0);
});

test('getStatusColor transitions green (<60%), amber (60-85%), pulsing red (>85%)', () => {
  assert.equal(getStatusColor(20), 'safe-green');
  assert.equal(getStatusColor(59.9), 'safe-green');
  assert.equal(getStatusColor(60), 'warning-amber');
  assert.equal(getStatusColor(85), 'warning-amber');
  assert.equal(getStatusColor(85.1), 'critical-pulsing-red');
  assert.equal(getStatusColor(120), 'critical-pulsing-red');
});

test('checkSystemStatus displays Exceeds Network Limit badge when over 100%', () => {
  const normal = { cpuUsage: 30, ramUsage: 40, footprintUsage: 20 };
  assert.equal(checkSystemStatus(normal), 'Active');

  const warning = { cpuUsage: 65, ramUsage: 40, footprintUsage: 20 };
  assert.equal(checkSystemStatus(warning), 'Active');

  const critical = { cpuUsage: 88, ramUsage: 40, footprintUsage: 20 };
  assert.equal(checkSystemStatus(critical), 'Critical');

  const exceeded = { cpuUsage: 105, ramUsage: 40, footprintUsage: 20 };
  assert.equal(checkSystemStatus(exceeded), 'Exceeds Network Limit');

  const exceededByRaw = {
    cpuUsage: 50,
    ramUsage: 50,
    footprintUsage: 50,
    rawCpu: 110_000_000,
  };
  assert.equal(checkSystemStatus(exceededByRaw), 'Exceeds Network Limit');
});
