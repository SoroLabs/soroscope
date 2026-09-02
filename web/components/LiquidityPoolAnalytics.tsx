'use client';

import React, { useState, useMemo } from 'react';
import {
  ResponsiveContainer,
  ComposedChart,
  Area,
  Line,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  CartesianGrid,
  Legend,
  ReferenceLine,
  PieChart,
  Pie,
  Cell,
} from 'recharts';
import {
  TrendingUp,
  TrendingDown,
  DollarSign,
  Percent,
  BarChart3,
  Calendar,
  Layers,
  Zap,
  Activity,
  TrendingUpIcon,
  AlertTriangle,
  PieChart as PieChartIcon,
  Clock,
} from 'lucide-react';

export type Timeframe = '7D' | '30D' | '90D' | '1Y' | 'ALL';

export interface PoolDataPoint {
  timestamp: number;
  date: string;
  apy: number;
  tvl: number;
  volume24h: number;
  fees24h: number;
  impermanentLoss: number;
  utilizationRate: number;
}

export interface PoolMetrics {
  feesApr: number;
  incentiveApr: number;
  totalApr: number;
  protocolFee: number;
  poolUtilization: number;
  impermanentLossPercent: number;
  volumeToTvlRatio: number;
  riskScore: 'low' | 'medium' | 'high';
  slippageBps: number;
  concentrationScore: number;
}

export interface LiquidityPoolAnalyticsProps {
  poolName?: string;
  tokenPair?: string;
  initialData?: PoolDataPoint[];
}

const TIMEFRAME_DAYS: Record<Timeframe, number> = {
  '7D': 7,
  '30D': 30,
  '90D': 90,
  '1Y': 365,
  ALL: 730,
};

export const generateMockPoolHistory = (totalDays: number = 365): PoolDataPoint[] => {
  const points: PoolDataPoint[] = [];
  const now = Date.now();
  const dayMs = 86400000;

  let baseApy = 14.5;
  let baseTvl = 2_500_000;
  let baseVolume = 450_000;
  let baseFees = 8_500;

  for (let i = totalDays; i >= 0; i--) {
    const timestamp = now - i * dayMs;
    const dateObj = new Date(timestamp);
    const dateStr = dateObj.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      ...(totalDays > 90 ? { year: '2-digit' } : {}),
    });

    const sinWave = Math.sin(i / 10) * 2.5;
    const noise = (Math.random() - 0.48) * 1.2;
    const apy = Math.max(1.5, parseFloat((baseApy + sinWave + noise).toFixed(2)));

    const tvlTrend = (totalDays - i) * 1200;
    const tvlNoise = (Math.random() - 0.5) * 50000;
    const tvl = Math.max(500000, Math.round(baseTvl + tvlTrend + tvlNoise));

    const volNoise = (Math.random() - 0.5) * 120000;
    const volume24h = Math.max(50000, Math.round(baseVolume + sinWave * 15000 + volNoise));

    const feesNoise = (Math.random() - 0.5) * 2500;
    const fees24h = Math.max(2000, Math.round(baseFees + sinWave * 800 + feesNoise));

    const ilNoise = (Math.random() - 0.5) * 0.8;
    const impermanentLoss = parseFloat((sinWave * 0.3 + ilNoise).toFixed(2));

    const utilizationNoise = (Math.random() - 0.5) * 8;
    const utilizationRate = Math.min(95, Math.max(15, 45 + sinWave * 5 + utilizationNoise));

    points.push({
      timestamp,
      date: dateStr,
      apy,
      tvl,
      volume24h,
      fees24h,
      impermanentLoss,
      utilizationRate,
    });
  }

  return points;
};

const formatCurrency = (value: number): string => {
  if (value >= 1_000_000) {
    return `$${(value / 1_000_000).toFixed(2)}M`;
  }
  if (value >= 1_000) {
    return `$${(value / 1_000).toFixed(1)}k`;
  }
  return `$${value.toLocaleString()}`;
};

const formatPercent = (value: number): string => `${value.toFixed(2)}%`;

export const LiquidityPoolAnalytics: React.FC<LiquidityPoolAnalyticsProps> = ({
  poolName = 'XLM / USDC Liquidity Pool',
  tokenPair = 'XLM-USDC',
  initialData,
}) => {
  const [timeframe, setTimeframe] = useState<Timeframe>('30D');
  const [activeMetrics, setActiveMetrics] = useState<{
    apy: boolean;
    tvl: boolean;
    volume: boolean;
    fees: boolean;
    utilization: boolean;
  }>({
    apy: true,
    tvl: true,
    volume: true,
    fees: false,
    utilization: false,
  });

  const rawData = useMemo(() => {
    return initialData || generateMockPoolHistory(365);
  }, [initialData]);

  const filteredData = useMemo(() => {
    const daysLimit = TIMEFRAME_DAYS[timeframe];
    if (rawData.length <= daysLimit) return rawData;
    return rawData.slice(rawData.length - daysLimit);
  }, [rawData, timeframe]);

  const summary = useMemo(() => {
    if (!filteredData.length) {
      return {
        currentApy: 0,
        apyChange: 0,
        currentTvl: 0,
        tvlChange: 0,
        currentVolume: 0,
        volumeChange: 0,
        peakApy: 0,
        currentFees: 0,
        feesApr: 0,
        incentiveApr: 0,
        totalApr: 0,
        protocolFee: 0.15,
        avgUtilization: 0,
        avgImpermanentLoss: 0,
        volumeToTvlRatio: 0,
        riskScore: 'medium' as const,
        slippageBps: 25,
        concentrationScore: 0,
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

    const currentFees = last.fees24h;
    const feesApr = parseFloat(((currentFees * 365) / currentTvl * 100).toFixed(2));
    const incentiveApr = parseFloat((currentApy - feesApr).toFixed(2));
    const totalApr = currentApy;

    const avgUtilization = parseFloat(
      (filteredData.reduce((sum, d) => sum + d.utilizationRate, 0) / filteredData.length).toFixed(1),
    );

    const avgImpermanentLoss = parseFloat(
      (filteredData.reduce((sum, d) => sum + Math.abs(d.impermanentLoss), 0) / filteredData.length).toFixed(2),
    );

    const volumeToTvlRatio = parseFloat(((currentVolume / currentTvl) * 100).toFixed(2));

    const riskScore =
      avgUtilization > 80 || tvlChange < -20 ? 'high' : avgUtilization > 60 || tvlChange < -10 ? 'medium' : 'low';

    const concentrationScore = parseFloat((100 - Math.abs(last.impermanentLoss) * 2).toFixed(1));

    return {
      currentApy,
      apyChange,
      currentTvl,
      tvlChange,
      currentVolume,
      volumeChange,
      peakApy,
      currentFees,
      feesApr,
      incentiveApr,
      totalApr,
      protocolFee: 0.15,
      avgUtilization,
      avgImpermanentLoss,
      volumeToTvlRatio,
      riskScore,
      slippageBps: 25,
      concentrationScore,
    };
  }, [filteredData]);

  const toggleMetric = (key: 'apy' | 'tvl' | 'volume' | 'fees' | 'utilization') => {
    setActiveMetrics((prev) => {
      const next = { ...prev, [key]: !prev[key] };
      if (!next.apy && !next.tvl && !next.volume && !next.fees && !next.utilization) return prev;
      return next;
    });
  };

  return (
    <div className="rounded-2xl border border-slate-800 bg-slate-900/80 p-6 shadow-xl backdrop-blur font-sans text-slate-100">
      {/* Pool Header & Interactive Controls */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between border-b border-slate-800 pb-5">
        <div>
          <div className="flex items-center gap-2">
            <span className="inline-flex items-center rounded-md bg-cyan-500/10 px-2.5 py-1 text-xs font-semibold text-cyan-400 border border-cyan-500/20">
              {tokenPair}
            </span>
            <h2 className="text-xl font-bold tracking-tight text-white">{poolName}</h2>
          </div>
          <p className="text-xs text-slate-400 mt-1">
            Historical APY yield rates, Total Value Locked, and 24h volume analytics
          </p>
        </div>

        {/* Timeframe Selectors */}
        <div className="flex items-center gap-1 rounded-xl bg-slate-950/80 p-1 border border-slate-800">
          {(['7D', '30D', '90D', '1Y', 'ALL'] as Timeframe[]).map((tf) => (
            <button
              key={tf}
              type="button"
              onClick={() => setTimeframe(tf)}
              className={`rounded-lg px-3 py-1.5 text-xs font-semibold transition-all ${
                timeframe === tf
                  ? 'bg-cyan-500 text-slate-950 shadow-md shadow-cyan-500/20 font-bold'
                  : 'text-slate-400 hover:text-white hover:bg-slate-800/60'
              }`}
            >
              {tf}
            </button>
          ))}
        </div>
      </div>

      {/* Summary KPI Cards */}
      <div className="grid grid-cols-1 gap-4 py-5 sm:grid-cols-2 lg:grid-cols-4">
        {/* Instant / Current APY */}
        <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-4 transition hover:border-slate-700">
          <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
            <span className="font-medium">Current APY</span>
            <Percent className="h-4 w-4 text-cyan-400" />
          </div>
          <div className="flex items-baseline justify-between">
            <span className="text-2xl font-black text-cyan-400">
              {formatPercent(summary.currentApy)}
            </span>
            <span
              className={`flex items-center text-xs font-bold ${
                summary.apyChange >= 0 ? 'text-emerald-400' : 'text-rose-400'
              }`}
            >
              {summary.apyChange >= 0 ? (
                <TrendingUp className="mr-0.5 h-3.5 w-3.5" />
              ) : (
                <TrendingDown className="mr-0.5 h-3.5 w-3.5" />
              )}
              {Math.abs(summary.apyChange).toFixed(1)}%
            </span>
          </div>
          <div className="mt-2 text-[11px] text-slate-500">
            Peak APY: <span className="text-slate-300 font-semibold">{formatPercent(summary.peakApy)}</span>
          </div>
        </div>

        {/* TVL */}
        <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-4 transition hover:border-slate-700">
          <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
            <span className="font-medium">Total Value Locked</span>
            <DollarSign className="h-4 w-4 text-emerald-400" />
          </div>
          <div className="flex items-baseline justify-between">
            <span className="text-2xl font-black text-emerald-400">
              {formatCurrency(summary.currentTvl)}
            </span>
            <span
              className={`flex items-center text-xs font-bold ${
                summary.tvlChange >= 0 ? 'text-emerald-400' : 'text-rose-400'
              }`}
            >
              {summary.tvlChange >= 0 ? (
                <TrendingUp className="mr-0.5 h-3.5 w-3.5" />
              ) : (
                <TrendingDown className="mr-0.5 h-3.5 w-3.5" />
              )}
              {Math.abs(summary.tvlChange).toFixed(1)}%
            </span>
          </div>
          <div className="mt-2 text-[11px] text-slate-500">
            Volume/TVL: <span className="text-slate-300 font-semibold">{summary.volumeToTvlRatio}%</span>
          </div>
        </div>

        {/* 24h Volume */}
        <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-4 transition hover:border-slate-700">
          <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
            <span className="font-medium">24h Volume</span>
            <BarChart3 className="h-4 w-4 text-purple-400" />
          </div>
          <div className="flex items-baseline justify-between">
            <span className="text-2xl font-black text-purple-400">
              {formatCurrency(summary.currentVolume)}
            </span>
            <span
              className={`flex items-center text-xs font-bold ${
                summary.volumeChange >= 0 ? 'text-emerald-400' : 'text-rose-400'
              }`}
            >
              {summary.volumeChange >= 0 ? (
                <TrendingUp className="mr-0.5 h-3.5 w-3.5" />
              ) : (
                <TrendingDown className="mr-0.5 h-3.5 w-3.5" />
              )}
              {Math.abs(summary.volumeChange).toFixed(1)}%
            </span>
          </div>
          <div className="mt-2 text-[11px] text-slate-500">
            24h Fees: <span className="text-slate-300 font-semibold">{formatCurrency(summary.currentFees)}</span>
          </div>
        </div>

        {/* APR Breakdown */}
        <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-4 transition hover:border-slate-700">
          <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
            <span className="font-medium">APR Breakdown</span>
            <Zap className="h-4 w-4 text-amber-400" />
          </div>
          <div className="space-y-1.5 mt-1">
            <div className="flex justify-between text-xs">
              <span className="text-slate-400">Fees APR:</span>
              <span className="text-emerald-400 font-medium">{formatPercent(summary.feesApr)}</span>
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-slate-400">Incentive:</span>
              <span className="text-cyan-400 font-medium">{formatPercent(summary.incentiveApr)}</span>
            </div>
            <div className="flex justify-between text-xs pt-1 border-t border-slate-800">
              <span className="text-slate-300 font-medium">Protocol Fee:</span>
              <span className="text-slate-400">{(summary.protocolFee * 100).toFixed(1)}%</span>
            </div>
          </div>
        </div>

        {/* Utilization Rate */}
        <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-4 transition hover:border-slate-700">
          <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
            <span className="font-medium">Pool Utilization</span>
            <Activity className="h-4 w-4 text-orange-400" />
          </div>
          <div className="flex items-baseline justify-between">
            <span className="text-2xl font-black text-orange-400">
              {summary.avgUtilization.toFixed(1)}%
            </span>
            <div className="w-16 h-1.5 bg-slate-800 rounded-full overflow-hidden">
              <div
                className={`h-full rounded-full transition-all ${
                  summary.avgUtilization > 80
                    ? 'bg-red-500'
                    : summary.avgUtilization > 60
                    ? 'bg-yellow-500'
                    : 'bg-emerald-500'
                }`}
                style={{ width: `${Math.min(summary.avgUtilization, 100)}%` }}
              />
            </div>
          </div>
          <div className="mt-2 text-[11px] text-slate-500">
            Avg IL: <span className="text-amber-400 font-semibold">{summary.avgImpermanentLoss.toFixed(2)}%</span>
          </div>
        </div>

        {/* Risk Score */}
        <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-4 transition hover:border-slate-700">
          <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
            <span className="font-medium">Risk Assessment</span>
            <AlertTriangle className={`h-4 w-4 ${
              summary.riskScore === 'high' ? 'text-red-400' :
              summary.riskScore === 'medium' ? 'text-yellow-400' : 'text-emerald-400'
            }`} />
          </div>
          <div className="flex items-baseline justify-between">
            <span className={`text-2xl font-black capitalize ${
              summary.riskScore === 'high' ? 'text-red-400' :
              summary.riskScore === 'medium' ? 'text-yellow-400' : 'text-emerald-400'
            }`}>
              {summary.riskScore}
            </span>
            <span className="text-xs text-slate-400">
              Slippage: <span className="text-slate-300">{summary.slippageBps} bps</span>
            </span>
          </div>
          <div className="mt-2 text-[11px] text-slate-500">
            Concentration: <span className="text-cyan-400 font-semibold">{summary.concentrationScore}%</span>
          </div>
        </div>

        {/* 24h Fees */}
        <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-4 transition hover:border-slate-700">
          <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
            <span className="font-medium">24h Fees Generated</span>
            <DollarSign className="h-4 w-4 text-green-400" />
          </div>
          <div className="flex items-baseline justify-between">
            <span className="text-2xl font-black text-green-400">
              {formatCurrency(summary.currentFees)}
            </span>
          </div>
          <div className="mt-2 text-[11px] text-slate-500">
            Fee Yield: <span className="text-slate-300 font-semibold">{formatPercent(summary.feesApr)}/yr</span>
          </div>
        </div>

        {/* Active Timeframe Info */}
        <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-4 transition hover:border-slate-700">
          <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
            <span className="font-medium">Selected Range</span>
            <Calendar className="h-4 w-4 text-amber-400" />
          </div>
          <div className="flex items-baseline justify-between">
            <span className="text-2xl font-black text-amber-400">{timeframe}</span>
            <span className="text-xs text-slate-400 font-semibold">
              {filteredData.length} points
            </span>
          </div>
          <div className="mt-2 text-[11px] text-slate-500">
            Granularity: 1-Day Aggregation
          </div>
        </div>
      </div>

      {/* Metric Visibility Toggles */}
      <div className="flex flex-wrap items-center justify-between gap-3 mb-4 rounded-xl bg-slate-950/40 p-3 border border-slate-800/80">
        <span className="text-xs font-medium text-slate-400 flex items-center gap-1.5">
          <Layers className="h-3.5 w-3.5 text-cyan-400" /> Display Metrics:
        </span>
        <div className="flex items-center gap-4 flex-wrap">
          <button
            type="button"
            onClick={() => toggleMetric('apy')}
            className={`flex items-center gap-1.5 text-xs font-medium transition ${
              activeMetrics.apy ? 'text-cyan-400 font-bold' : 'text-slate-500 opacity-60'
            }`}
          >
            <span className="h-2.5 w-2.5 rounded-full bg-cyan-400" /> APY %
          </button>
          <button
            type="button"
            onClick={() => toggleMetric('tvl')}
            className={`flex items-center gap-1.5 text-xs font-medium transition ${
              activeMetrics.tvl ? 'text-emerald-400 font-bold' : 'text-slate-500 opacity-60'
            }`}
          >
            <span className="h-2.5 w-2.5 rounded-full bg-emerald-400" /> TVL ($)
          </button>
          <button
            type="button"
            onClick={() => toggleMetric('volume')}
            className={`flex items-center gap-1.5 text-xs font-medium transition ${
              activeMetrics.volume ? 'text-purple-400 font-bold' : 'text-slate-500 opacity-60'
            }`}
          >
            <span className="h-2.5 w-2.5 rounded-full bg-purple-400" /> Volume ($)
          </button>
          <button
            type="button"
            onClick={() => toggleMetric('fees')}
            className={`flex items-center gap-1.5 text-xs font-medium transition ${
              activeMetrics.fees ? 'text-green-400 font-bold' : 'text-slate-500 opacity-60'
            }`}
          >
            <span className="h-2.5 w-2.5 rounded-full bg-green-400" /> Fees ($)
          </button>
          <button
            type="button"
            onClick={() => toggleMetric('utilization')}
            className={`flex items-center gap-1.5 text-xs font-medium transition ${
              activeMetrics.utilization ? 'text-orange-400 font-bold' : 'text-slate-500 opacity-60'
            }`}
          >
            <span className="h-2.5 w-2.5 rounded-full bg-orange-400" /> Utilization
          </button>
        </div>
      </div>

      {/* Main Recharts Visualizer */}
      <div className="h-[360px] w-full pt-2">
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart
            data={filteredData}
            margin={{ top: 10, right: 20, left: 10, bottom: 0 }}
          >
            <defs>
              <linearGradient id="colorApy" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="#06b6d4" stopOpacity={0.4} />
                <stop offset="95%" stopColor="#06b6d4" stopOpacity={0.0} />
              </linearGradient>
              <linearGradient id="colorTvl" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="#10b981" stopOpacity={0.3} />
                <stop offset="95%" stopColor="#10b981" stopOpacity={0.0} />
              </linearGradient>
            </defs>

            <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" vertical={false} />

            <XAxis
              dataKey="date"
              stroke="#64748b"
              fontSize={11}
              tickLine={false}
              axisLine={{ stroke: '#334155' }}
            />

            {/* Left Y Axis for APY (%) */}
            {activeMetrics.apy && (
              <YAxis
                yAxisId="left"
                stroke="#06b6d4"
                fontSize={11}
                tickLine={false}
                axisLine={false}
                tickFormatter={(v: number) => `${v}%`}
                domain={['auto', 'auto']}
              />
            )}

            {/* Right Y Axis for Currency ($ TVL & Volume) */}
            {(activeMetrics.tvl || activeMetrics.volume) && (
              <YAxis
                yAxisId="right"
                orientation="right"
                stroke="#10b981"
                fontSize={11}
                tickLine={false}
                axisLine={false}
                tickFormatter={(v: number) => formatCurrency(v)}
                domain={['auto', 'auto']}
              />
            )}

            <Tooltip
              contentStyle={{
                backgroundColor: '#090d16',
                borderColor: '#1e293b',
                borderRadius: '0.75rem',
                color: '#f8fafc',
                fontSize: '12px',
                boxShadow: '0 10px 15px -3px rgba(0, 0, 0, 0.5)',
              }}
              formatter={(value: any, name: any) => {
                const val = Number(value);
                if (name === 'APY Rate (%)') return [formatPercent(val), name];
                if (name === 'Total Value Locked') return [formatCurrency(val), name];
                if (name === '24h Volume') return [formatCurrency(val), name];
                return [val, name];
              }}
            />

            <Legend
              wrapperStyle={{ paddingTop: '12px', fontSize: '12px' }}
              iconType="circle"
            />

            {/* Volume Bars */}
            {activeMetrics.volume && (
              <Bar
                yAxisId="right"
                dataKey="volume24h"
                name="24h Volume"
                fill="#a855f7"
                opacity={0.35}
                radius={[4, 4, 0, 0]}
              />
            )}

            {/* TVL Area */}
            {activeMetrics.tvl && (
              <Area
                yAxisId="right"
                type="monotone"
                dataKey="tvl"
                name="Total Value Locked"
                stroke="#10b981"
                strokeWidth={2}
                fillOpacity={1}
                fill="url(#colorTvl)"
              />
            )}

            {/* APY Line */}
            {activeMetrics.apy && (
              <Line
                yAxisId="left"
                type="monotone"
                dataKey="apy"
                name="APY Rate (%)"
                stroke="#06b6d4"
                strokeWidth={3}
                dot={false}
                activeDot={{ r: 6, fill: '#06b6d4', stroke: '#090d16', strokeWidth: 2 }}
              />
            )}

            {/* Fees Line */}
            {activeMetrics.fees && (
              <Line
                yAxisId="right"
                type="monotone"
                dataKey="fees24h"
                name="24h Fees"
                stroke="#22c55e"
                strokeWidth={2}
                dot={false}
                strokeDasharray="5 5"
              />
            )}

            {/* Utilization Line */}
            {activeMetrics.utilization && (
              <Line
                yAxisId="right"
                type="monotone"
                dataKey="utilizationRate"
                name="Utilization %"
                stroke="#f97316"
                strokeWidth={2}
                dot={false}
              />
            )}
          </ComposedChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
};
