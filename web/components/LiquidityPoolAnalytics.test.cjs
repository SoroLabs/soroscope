'use strict';

const { describe, test } = require('node:test');
const assert = require('node:assert/strict');

// Import helper logic or recreate dataset helper for unit testing
const TIMEFRAME_DAYS = {
  '7D': 7,
  '30D': 30,
  '90D': 90,
  '1Y': 365,
  ALL: 730,
};

const generateMockPoolHistory = (totalDays = 365) => {
  const points = [];
  const now = Date.now();
  const dayMs = 86400000;

  let baseApy = 14.5;
  let baseTvl = 2500000;
  let baseVolume = 450000;

  for (let i = totalDays; i >= 0; i--) {
    const timestamp = now - i * dayMs;
    const dateObj = new Date(timestamp);
    const dateStr = dateObj.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
    });

    const sinWave = Math.sin(i / 10) * 2.5;
    const apy = Math.max(1.5, parseFloat((baseApy + sinWave).toFixed(2)));
    const tvl = Math.max(500000, Math.round(baseTvl + (totalDays - i) * 1200));
    const volume24h = Math.max(50000, Math.round(baseVolume + sinWave * 15000));

    points.push({
      timestamp,
      date: dateStr,
      apy,
      tvl,
      volume24h,
    });
  }

  return points;
};

const filterDataByTimeframe = (rawData, timeframe) => {
  const daysLimit = TIMEFRAME_DAYS[timeframe];
  if (!daysLimit || rawData.length <= daysLimit) return rawData;
  return rawData.slice(rawData.length - daysLimit);
};

const calculateSummaryMetrics = (filteredData) => {
  if (!filteredData || !filteredData.length) {
    return {
      currentApy: 0,
      apyChange: 0,
      currentTvl: 0,
      tvlChange: 0,
      currentVolume: 0,
      volumeChange: 0,
      peakApy: 0,
    };
  }

  const first = filteredData[0];
  const last = filteredData[filteredData.length - 1];

  const currentApy = last.apy;
  const apyChange = first.apy ? ((last.apy - first.apy) / first.apy) * 100 : 0;

  const currentTvl = last.tvl;
  const tvlChange = first.tvl ? ((last.tvl - first.tvl) / first.tvl) * 100 : 0;

  const currentVolume = last.volume24h;
  const volumeChange = first.volume24h
    ? ((last.volume24h - first.volume24h) / first.volume24h) * 100
    : 0;

  const peakApy = Math.max(...filteredData.map((d) => d.apy));

  return {
    currentApy,
    apyChange,
    currentTvl,
    tvlChange,
    currentVolume,
    volumeChange,
    peakApy,
  };
};

describe('LiquidityPoolAnalytics Unit Tests', () => {
  test('generateMockPoolHistory creates requested number of daily points', () => {
    const history = generateMockPoolHistory(30);
    assert.strictEqual(history.length, 31);
    assert.ok(history[0].timestamp < history[history.length - 1].timestamp);
    assert.ok(typeof history[0].apy === 'number');
    assert.ok(typeof history[0].tvl === 'number');
    assert.ok(typeof history[0].volume24h === 'number');
  });

  test('filterDataByTimeframe filters datasets correctly for select timeframes', () => {
    const rawData = generateMockPoolHistory(365);
    
    const data7d = filterDataByTimeframe(rawData, '7D');
    assert.strictEqual(data7d.length, 7);

    const data30d = filterDataByTimeframe(rawData, '30D');
    assert.strictEqual(data30d.length, 30);

    const data90d = filterDataByTimeframe(rawData, '90D');
    assert.strictEqual(data90d.length, 90);

    const dataAll = filterDataByTimeframe(rawData, 'ALL');
    assert.strictEqual(dataAll.length, 366);
  });

  test('calculateSummaryMetrics computes accurate KPI changes and peak values', () => {
    const mockPoints = [
      { timestamp: 1000, date: 'Jan 1', apy: 10.0, tvl: 100000, volume24h: 20000 },
      { timestamp: 2000, date: 'Jan 2', apy: 15.0, tvl: 120000, volume24h: 25000 },
      { timestamp: 3000, date: 'Jan 3', apy: 12.0, tvl: 150000, volume24h: 30000 },
    ];

    const summary = calculateSummaryMetrics(mockPoints);

    assert.strictEqual(summary.currentApy, 12.0);
    assert.strictEqual(summary.peakApy, 15.0);
    assert.strictEqual(summary.currentTvl, 150000);
    assert.strictEqual(summary.currentVolume, 30000);

    // APY change from 10.0 to 12.0 is +20%
    assert.strictEqual(summary.apyChange, 20.0);

    // TVL change from 100000 to 150000 is +50%
    assert.strictEqual(summary.tvlChange, 50.0);

    // Volume change from 20000 to 30000 is +50%
    assert.strictEqual(summary.volumeChange, 50.0);
  });

  test('calculateSummaryMetrics handles empty data safely', () => {
    const summary = calculateSummaryMetrics([]);
    assert.strictEqual(summary.currentApy, 0);
    assert.strictEqual(summary.peakApy, 0);
    assert.strictEqual(summary.currentTvl, 0);
    assert.strictEqual(summary.currentVolume, 0);
  });
});
