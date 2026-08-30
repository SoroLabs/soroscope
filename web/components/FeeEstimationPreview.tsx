'use client';

import React, { useState, useEffect, useCallback } from 'react';
import { Fuel, Zap, TrendingUp, Gauge, DollarSign } from 'lucide-react';
import type { FeeEstimate, FeeBumpOption } from '../lib/sorobantypes';
import { useNetwork } from '../context/NetworkContext';
import { fetchFeeStats, estimateFees, stroopsToXlm } from '../lib/stellarRpc';

interface FeeEstimationPreviewProps {
  costStroops?: number;
  loading?: boolean;
  onFeeBumpChange?: (level: string, feeStroops: number) => void;
}

export function FeeEstimationPreview({
  costStroops = 0,
  loading = false,
  onFeeBumpChange,
}: FeeEstimationPreviewProps) {
  const { network } = useNetwork();
  const [feeEstimate, setFeeEstimate] = useState<FeeEstimate | null>(null);
  const [selectedBump, setSelectedBump] = useState<string>('low');
  const [fetching, setFetching] = useState(false);
  const [rpcAvailable, setRpcAvailable] = useState(true);

  const loadFeeEstimate = useCallback(async () => {
    if (costStroops <= 0) {
      setFeeEstimate(null);
      return;
    }
    setFetching(true);
    try {
      const stats = await fetchFeeStats(network.rpcUrl);
      setRpcAvailable(true);
      const estimate = estimateFees(costStroops, stats);
      setFeeEstimate(estimate);
    } catch {
      const estimate = estimateFees(costStroops, null);
      setFeeEstimate(estimate);
      setRpcAvailable(false);
    } finally {
      setFetching(false);
    }
  }, [costStroops, network.rpcUrl]);

  useEffect(() => {
    loadFeeEstimate();
  }, [loadFeeEstimate]);

  const handleBumpSelect = (bump: FeeBumpOption) => {
    setSelectedBump(bump.label);
    if (onFeeBumpChange) {
      const level = bump.label.toLowerCase().includes('low') ? 'low'
        : bump.label.toLowerCase().includes('high') ? 'high'
        : 'medium';
      onFeeBumpChange(level, bump.feeStroops);
    }
  };

  const isBusy = loading || fetching;
  const hasFees = feeEstimate && feeEstimate.totalFeeStroops > 0;

  return (
    <div className="rounded-2xl border border-slate-800 bg-slate-900/70 p-5 font-mono">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <Fuel className="h-4 w-4 text-cyan-400" />
          <h3 className="text-sm font-bold text-slate-100 uppercase tracking-wider">
            Network Fee Estimation
          </h3>
        </div>
        {!rpcAvailable && (
          <span className="text-[10px] text-amber-400 bg-amber-500/10 border border-amber-500/30 rounded px-2 py-0.5">
            Estimated (RPC offline)
          </span>
        )}
      </div>

      {isBusy ? (
        <div className="flex items-center justify-center py-8">
          <div className="h-5 w-5 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin" />
        </div>
      ) : hasFees ? (
        <div className="space-y-4">
          <div className="grid grid-cols-3 gap-3">
            <div className="bg-slate-950/60 border border-slate-800 rounded-lg p-3 text-center">
              <div className="flex items-center justify-center gap-1 text-[10px] text-slate-500 uppercase mb-1">
                <Zap className="h-3 w-3" />
                Resource Fee
              </div>
              <div className="text-sm font-bold text-cyan-400">
                {stroopsToXlm(feeEstimate.minResourceFeeStroops)} XLM
              </div>
              <div className="text-[9px] text-slate-500 mt-0.5">
                {feeEstimate.minResourceFeeStroops.toLocaleString()} stroops
              </div>
            </div>
            <div className="bg-slate-950/60 border border-slate-800 rounded-lg p-3 text-center">
              <div className="flex items-center justify-center gap-1 text-[10px] text-slate-500 uppercase mb-1">
                <Gauge className="h-3 w-3" />
                Classic Fee
              </div>
              <div className="text-sm font-bold text-slate-100">
                {stroopsToXlm(feeEstimate.classicFeeStroops)} XLM
              </div>
              <div className="text-[9px] text-slate-500 mt-0.5">
                {feeEstimate.classicFeeStroops.toLocaleString()} stroops
              </div>
            </div>
            <div className="bg-slate-950/60 border border-slate-800 rounded-lg p-3 text-center">
              <div className="flex items-center justify-center gap-1 text-[10px] text-slate-500 uppercase mb-1">
                <DollarSign className="h-3 w-3" />
                Total
              </div>
              <div className="text-sm font-bold text-emerald-400">
                {feeEstimate.totalFeeXlm} XLM
              </div>
              <div className="text-[9px] text-slate-500 mt-0.5">
                {feeEstimate.totalFeeStroops.toLocaleString()} stroops
              </div>
            </div>
          </div>

          {feeEstimate.surgeMultiplier > 1 && (
            <div className="flex items-center gap-2 text-[11px] text-amber-400 bg-amber-500/5 border border-amber-500/20 rounded-lg px-3 py-2">
              <TrendingUp className="h-3.5 w-3.5 shrink-0" />
              <span>
                Network surge active ({feeEstimate.surgeMultiplier}x multiplier) &mdash; fees may be higher than usual
              </span>
            </div>
          )}

          <div>
            <div className="text-[10px] text-slate-500 uppercase mb-2">Fee Bump Options</div>
            <div className="grid grid-cols-3 gap-2">
              {feeEstimate.feeBumps.map((bump) => {
                const isSelected = selectedBump === bump.label;
                return (
                  <button
                    key={bump.label}
                    type="button"
                    disabled={isBusy}
                    onClick={() => handleBumpSelect(bump)}
                    className={`rounded-lg border px-3 py-2.5 text-left transition-all duration-150 ${
                      isSelected
                        ? 'border-cyan-500/50 bg-cyan-500/10 ring-1 ring-cyan-500/30'
                        : 'border-slate-800 bg-slate-950/40 hover:border-slate-700 hover:bg-slate-950/60'
                    }`}
                  >
                    <div className={`text-xs font-semibold ${isSelected ? 'text-cyan-400' : 'text-slate-300'}`}>
                      {bump.label}
                    </div>
                    <div className={`text-sm font-bold mt-1 ${isSelected ? 'text-white' : 'text-slate-100'}`}>
                      {bump.feeXlm} XLM
                    </div>
                    <div className="text-[9px] text-slate-500 mt-0.5">
                      {bump.feeStroops.toLocaleString()} stroops
                    </div>
                  </button>
                );
              })}
            </div>
          </div>

          <div className="border-t border-slate-800 pt-3">
            <div className="flex items-center justify-between text-[10px] text-slate-500">
              <span>Surge multiplier: {feeEstimate.surgeMultiplier}x</span>
              <button
                type="button"
                onClick={loadFeeEstimate}
                disabled={isBusy}
                className="text-cyan-400 hover:text-cyan-300 transition-colors cursor-pointer"
              >
                Refresh
              </button>
            </div>
          </div>
        </div>
      ) : (
        <div className="text-center py-6">
          <Fuel className="h-8 w-8 text-slate-700 mx-auto mb-2" />
          <p className="text-xs text-slate-500">
            Run a simulation to see network fee estimates
          </p>
        </div>
      )}
    </div>
  );
}
