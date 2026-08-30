import React, { useState, useMemo } from 'react';
import { calculateStakingYield } from '../lib/stakingCalculator';
import { Calculator, TrendingUp, Calendar, Zap, DollarSign, RefreshCw, BarChart2 } from 'lucide-react';

export interface StakingCalculatorProps {
  initialDeposit?: number;
  initialDurationMonths?: number;
  initialBaseApy?: number;
  className?: string;
}

export function StakingCalculator({
  initialDeposit = 1000,
  initialDurationMonths = 12,
  initialBaseApy = 12,
  className = '',
}: StakingCalculatorProps) {
  const [depositAmount, setDepositAmount] = useState<number>(initialDeposit);
  const [lockDurationMonths, setLockDurationMonths] = useState<number>(initialDurationMonths);
  const [baseApyPercentage, setBaseApyPercentage] = useState<number>(initialBaseApy);
  const [compoundFrequency, setCompoundFrequency] = useState<string>('monthly');
  const [enableTierMultiplier, setEnableTierMultiplier] = useState<boolean>(true);
  const [showTable, setShowTable] = useState<boolean>(false);

  const results = useMemo(() => {
    return calculateStakingYield({
      depositAmount,
      lockDurationMonths,
      compoundFrequency,
      baseApyPercentage,
      enableTierMultiplier,
    });
  }, [depositAmount, lockDurationMonths, compoundFrequency, baseApyPercentage, enableTierMultiplier]);

  const presetAmounts = [500, 1000, 5000, 10000, 50000];
  const presetDurations = [
    { label: '1M', months: 1 },
    { label: '3M', months: 3 },
    { label: '6M', months: 6 },
    { label: '12M (1Y)', months: 12 },
    { label: '24M (2Y)', months: 24 },
    { label: '36M (3Y)', months: 36 },
  ];

  return (
    <div className={`rounded-2xl border border-slate-800 bg-slate-900/90 p-6 shadow-2xl backdrop-blur ${className}`}>
      {/* Widget Header */}
      <div className="mb-6 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between border-b border-slate-800 pb-4">
        <div>
          <div className="flex items-center gap-2">
            <Calculator className="h-6 w-6 text-cyan-400" />
            <h2 className="text-xl font-bold text-slate-100">Token Yield & Staking Calculator</h2>
          </div>
          <p className="mt-1 text-sm text-slate-400">
            Estimate projected APY, compounding returns, and rewards prior to locking your tokens.
          </p>
        </div>
        <div className="inline-flex items-center gap-2 rounded-full border border-cyan-500/30 bg-cyan-950/40 px-3 py-1 text-xs font-semibold text-cyan-400">
          <Zap className="h-3.5 w-3.5" />
          <span>Soroban Staking</span>
        </div>
      </div>

      <div className="grid gap-8 lg:grid-cols-12">
        {/* Left Column: Interactive Input Controls */}
        <div className="space-y-6 lg:col-span-7">
          {/* Deposit Amount Input */}
          <div className="space-y-2">
            <div className="flex items-center justify-between text-sm font-medium">
              <label htmlFor="deposit-amount-input" className="text-slate-300">
                Deposit Amount
              </label>
              <div className="flex items-center gap-1 font-mono text-cyan-400">
                <DollarSign className="h-4 w-4" />
                <input
                  id="deposit-amount-input"
                  type="number"
                  min="0"
                  max="1000000"
                  value={depositAmount}
                  onChange={(e) => setDepositAmount(Math.max(0, Number(e.target.value)))}
                  className="w-28 rounded border border-slate-700 bg-slate-950 px-2 py-0.5 text-right font-mono text-slate-100 focus:border-cyan-400 focus:outline-none"
                />
              </div>
            </div>
            <input
              type="range"
              min="10"
              max="100000"
              step="50"
              value={depositAmount}
              onChange={(e) => setDepositAmount(Number(e.target.value))}
              className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-slate-800 accent-cyan-400"
            />
            {/* Quick Amount Presets */}
            <div className="flex flex-wrap gap-1.5 pt-1">
              {presetAmounts.map((amt) => (
                <button
                  key={amt}
                  type="button"
                  onClick={() => setDepositAmount(amt)}
                  className={`rounded px-2.5 py-1 text-xs font-medium transition ${
                    depositAmount === amt
                      ? 'bg-cyan-500 text-slate-950 font-semibold'
                      : 'bg-slate-800 text-slate-300 hover:bg-slate-700'
                  }`}
                >
                  {amt.toLocaleString()}
                </button>
              ))}
            </div>
          </div>

          {/* Lock Duration Slider & Presets */}
          <div className="space-y-2">
            <div className="flex items-center justify-between text-sm font-medium">
              <label htmlFor="lock-duration-slider" className="text-slate-300">
                Lock Duration: <span className="font-mono text-cyan-400">{lockDurationMonths} Months</span>
              </label>
              {enableTierMultiplier && (
                <span className="text-xs font-medium text-emerald-400">
                  {results.multiplier}x Duration Multiplier
                </span>
              )}
            </div>
            <input
              id="lock-duration-slider"
              type="range"
              min="1"
              max="36"
              step="1"
              value={lockDurationMonths}
              onChange={(e) => setLockDurationMonths(Number(e.target.value))}
              className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-slate-800 accent-cyan-400"
            />
            {/* Duration Presets */}
            <div className="flex flex-wrap gap-1.5 pt-1">
              {presetDurations.map((item) => (
                <button
                  key={item.months}
                  type="button"
                  onClick={() => setLockDurationMonths(item.months)}
                  className={`rounded px-2.5 py-1 text-xs font-medium transition ${
                    lockDurationMonths === item.months
                      ? 'bg-cyan-500 text-slate-950 font-semibold'
                      : 'bg-slate-800 text-slate-300 hover:bg-slate-700'
                  }`}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>

          {/* Base APY (%) Slider */}
          <div className="space-y-2">
            <div className="flex items-center justify-between text-sm font-medium">
              <label htmlFor="base-apy-input" className="text-slate-300">
                Base APY (%)
              </label>
              <div className="flex items-center gap-1 font-mono text-cyan-400">
                <input
                  id="base-apy-input"
                  type="number"
                  min="0"
                  max="100"
                  step="0.1"
                  value={baseApyPercentage}
                  onChange={(e) => setBaseApyPercentage(Math.max(0, Number(e.target.value)))}
                  className="w-20 rounded border border-slate-700 bg-slate-950 px-2 py-0.5 text-right font-mono text-slate-100 focus:border-cyan-400 focus:outline-none"
                />
                <span>%</span>
              </div>
            </div>
            <input
              type="range"
              min="1"
              max="50"
              step="0.5"
              value={baseApyPercentage}
              onChange={(e) => setBaseApyPercentage(Number(e.target.value))}
              className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-slate-800 accent-cyan-400"
            />
          </div>

          {/* Compound Frequency Select Pills */}
          <div className="space-y-2">
            <label className="block text-sm font-medium text-slate-300">Compound Frequency</label>
            <div className="grid grid-cols-3 gap-2 sm:grid-cols-6">
              {[
                { id: 'daily', label: 'Daily' },
                { id: 'weekly', label: 'Weekly' },
                { id: 'monthly', label: 'Monthly' },
                { id: 'quarterly', label: 'Quarterly' },
                { id: 'annually', label: 'Annual' },
                { id: 'none', label: 'Simple' },
              ].map((freq) => (
                <button
                  key={freq.id}
                  type="button"
                  onClick={() => setCompoundFrequency(freq.id)}
                  className={`rounded-lg border px-2 py-2 text-center text-xs font-medium transition ${
                    compoundFrequency === freq.id
                      ? 'border-cyan-500 bg-cyan-950/60 text-cyan-300'
                      : 'border-slate-800 bg-slate-950/40 text-slate-400 hover:border-slate-700 hover:text-slate-200'
                  }`}
                >
                  {freq.label}
                </button>
              ))}
            </div>
          </div>

          {/* Lock Duration Tier Multiplier Toggle */}
          <div className="flex items-center justify-between rounded-xl border border-slate-800 bg-slate-950/50 p-3">
            <div className="flex items-center gap-2">
              <TrendingUp className="h-4 w-4 text-emerald-400" />
              <div>
                <span className="text-xs font-semibold text-slate-200">Lock Duration Tier Multiplier</span>
                <p className="text-[11px] text-slate-400">Bonus APY for locking tokens for longer periods</p>
              </div>
            </div>
            <button
              type="button"
              onClick={() => setEnableTierMultiplier(!enableTierMultiplier)}
              className={`relative inline-flex h-5 w-10 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
                enableTierMultiplier ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out ${
                  enableTierMultiplier ? 'translate-x-5' : 'translate-x-0'
                }`}
              />
            </button>
          </div>
        </div>

        {/* Right Column: Projected Results & Summary Cards */}
        <div className="flex flex-col justify-between space-y-4 lg:col-span-5">
          {/* Main Projection Cards */}
          <div className="grid gap-3">
            {/* Projected Total Balance */}
            <div className="rounded-xl border border-slate-800 bg-gradient-to-br from-slate-900 to-slate-950 p-4">
              <div className="flex items-center justify-between text-xs font-medium text-slate-400">
                <span>Projected Total Balance</span>
                <Calendar className="h-4 w-4 text-cyan-400" />
              </div>
              <div className="mt-2 flex items-baseline gap-2">
                <span className="text-3xl font-bold font-mono text-cyan-300">
                  {results.totalBalance.toLocaleString()}
                </span>
                <span className="text-xs font-semibold text-slate-400">Tokens</span>
              </div>
              <div className="mt-2 text-xs text-slate-400">
                Initial: <span className="font-mono text-slate-300">{results.depositAmount.toLocaleString()}</span>
              </div>
            </div>

            {/* Total Staking Rewards */}
            <div className="rounded-xl border border-slate-800 bg-gradient-to-br from-slate-900 to-slate-950 p-4">
              <div className="flex items-center justify-between text-xs font-medium text-slate-400">
                <span>Total Staking Rewards</span>
                <Zap className="h-4 w-4 text-emerald-400" />
              </div>
              <div className="mt-2 flex items-baseline gap-2">
                <span className="text-2xl font-bold font-mono text-emerald-400">
                  +{results.totalInterest.toLocaleString()}
                </span>
                <span className="text-xs font-semibold text-slate-400">Tokens</span>
              </div>
              <div className="mt-2 flex items-center justify-between text-xs text-slate-400">
                <span>Total ROI:</span>
                <span className="font-mono font-semibold text-emerald-400">+{results.totalRoiPercent}%</span>
              </div>
            </div>

            {/* Effective APY Card */}
            <div className="grid grid-cols-2 gap-3">
              <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-3">
                <span className="text-[11px] font-medium text-slate-400">Effective APY</span>
                <div className="mt-1 text-lg font-bold font-mono text-cyan-400">
                  {results.effectiveApyPercent}%
                </div>
                <span className="text-[10px] text-slate-500">Base: {results.baseApyPercentage}%</span>
              </div>
              <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-3">
                <span className="text-[11px] font-medium text-slate-400">Monthly Reward</span>
                <div className="mt-1 text-lg font-bold font-mono text-emerald-400">
                  +{results.estimatedMonthlyYield.toLocaleString()}
                </div>
                <span className="text-[10px] text-slate-500">~{results.estimatedDailyYield} / day</span>
              </div>
            </div>
          </div>

          {/* Toggle Projection Schedule Table */}
          <div className="pt-2">
            <button
              type="button"
              onClick={() => setShowTable(!showTable)}
              className="flex w-full items-center justify-center gap-2 rounded-xl border border-slate-800 bg-slate-950 py-2.5 text-xs font-semibold text-slate-300 hover:bg-slate-800 transition"
            >
              <BarChart2 className="h-4 w-4 text-cyan-400" />
              <span>{showTable ? 'Hide Monthly Milestone Schedule' : 'View Monthly Milestone Schedule'}</span>
            </button>
          </div>
        </div>
      </div>

      {/* Monthly Milestone Table */}
      {showTable && (
        <div className="mt-6 border-t border-slate-800 pt-4">
          <h3 className="mb-3 text-sm font-semibold text-slate-200">
            Monthly Compounding Schedule ({results.lockDurationMonths} Months)
          </h3>
          <div className="max-h-60 overflow-y-auto rounded-xl border border-slate-800 bg-slate-950">
            <table className="w-full text-left text-xs font-mono">
              <thead className="sticky top-0 bg-slate-900 text-slate-400">
                <tr>
                  <th className="px-4 py-2">Month</th>
                  <th className="px-4 py-2">Cumulative Yield</th>
                  <th className="px-4 py-2 text-right">Projected Balance</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800/50 text-slate-300">
                {results.breakdownByMonth.map((row) => (
                  <tr key={row.month} className="hover:bg-slate-900/50">
                    <td className="px-4 py-2">Month {row.month}</td>
                    <td className="px-4 py-2 text-emerald-400">+{row.yieldEarned.toLocaleString()}</td>
                    <td className="px-4 py-2 text-right font-bold text-slate-100">
                      {row.balance.toLocaleString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
