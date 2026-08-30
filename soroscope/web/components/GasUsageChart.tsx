'use client';

import React from 'react';
import { BarChart3, Cpu, Database, HardDrive, Activity, DollarSign } from 'lucide-react';
import type { TestnetAverages } from '../lib/sorobantypes';

interface GasMetric {
  key: string;
  label: string;
  simulated: number;
  average: number;
  unit: string;
  color: string;
  icon: React.ElementType;
}

const DEFAULT_TESTNET_AVERAGES: TestnetAverages = {
  cpu_instructions: 3_000_000,
  ram_bytes: 512_000,
  ledger_read_bytes: 2_048,
  ledger_write_bytes: 1_024,
  transaction_size_bytes: 600,
};

interface GasUsageChartProps {
  cpu_instructions: number;
  ram_bytes: number;
  ledger_read_bytes: number;
  ledger_write_bytes: number;
  transaction_size_bytes: number;
  cost_stroops?: number;
  testnetAverages?: TestnetAverages;
}

const formatCompact = (num: number) =>
  new Intl.NumberFormat('en-US', { notation: 'compact', compactDisplay: 'short' }).format(num);

const formatStroops = (stroops: number) => {
  const xlm = stroops / 10_000_000;
  return `${xlm.toFixed(7)} XLM`;
};

export const GasUsageChart: React.FC<GasUsageChartProps> = ({
  cpu_instructions,
  ram_bytes,
  ledger_read_bytes,
  ledger_write_bytes,
  transaction_size_bytes,
  cost_stroops,
  testnetAverages,
}) => {
  const avg = testnetAverages ?? DEFAULT_TESTNET_AVERAGES;

  const metrics: GasMetric[] = [
    {
      key: 'cpu',
      label: 'CPU Instructions',
      simulated: cpu_instructions,
      average: avg.cpu_instructions,
      unit: 'instr',
      color: '#ef4444',
      icon: Cpu,
    },
    {
      key: 'ram',
      label: 'Memory (RAM)',
      simulated: ram_bytes,
      average: avg.ram_bytes,
      unit: 'bytes',
      color: '#eab308',
      icon: Activity,
    },
    {
      key: 'reads',
      label: 'Ledger Reads',
      simulated: ledger_read_bytes,
      average: avg.ledger_read_bytes,
      unit: 'bytes',
      color: '#3b82f6',
      icon: Database,
    },
    {
      key: 'writes',
      label: 'Ledger Writes',
      simulated: ledger_write_bytes,
      average: avg.ledger_write_bytes,
      unit: 'bytes',
      color: '#10b981',
      icon: HardDrive,
    },
    {
      key: 'txsize',
      label: 'Transaction Size',
      simulated: transaction_size_bytes,
      average: avg.transaction_size_bytes,
      unit: 'bytes',
      color: '#a371f7',
      icon: Activity,
    },
  ];

  const maxValue = Math.max(
    ...metrics.map((m) => Math.max(m.simulated, m.average)),
    1,
  );

  return (
    <div className="bg-[#161b22] border border-[#30363d] rounded-lg p-6 font-mono">
      <div className="border-b-2 border-[#30363d] pb-2 mb-4 flex justify-between items-end">
        <div className="flex items-center gap-2">
          <BarChart3 size={18} className="text-[#8b949e]" />
          <h2 className="text-2xl font-black text-[#c9d1d9] uppercase tracking-wider">
            Gas Usage vs Testnet Avg
          </h2>
        </div>
        <span className="text-xs text-[#8b949e]">Per Transaction</span>
      </div>

      {/* Legend */}
      <div className="flex gap-6 mb-6 text-xs">
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: '#00d9ff' }} />
          <span className="text-[#c9d1d9]">Simulated</span>
        </div>
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: '#8b949e' }} />
          <span className="text-[#8b949e]">Testnet Avg</span>
        </div>
      </div>

      <div className="space-y-5">
        {metrics.map((metric) => {
          const simulatedPct = (metric.simulated / maxValue) * 100;
          const averagePct = (metric.average / maxValue) * 100;

          return (
            <div key={metric.key}>
              {/* Label Row */}
              <div className="flex justify-between items-center mb-1">
                <div className="flex items-center gap-2 text-[#c9d1d9]">
                  <metric.icon size={14} className="text-[#8b949e]" />
                  <span className="font-bold text-sm">{metric.label}</span>
                </div>
                <div className="flex gap-4 text-xs">
                  <span className="text-[#00d9ff] font-semibold">
                    {formatCompact(metric.simulated)}
                    <span className="text-[#8b949e] font-normal ml-1">{metric.unit}</span>
                  </span>
                  <span className="text-[#8b949e]">
                    avg {formatCompact(metric.average)}
                    <span className="ml-1">{metric.unit}</span>
                  </span>
                </div>
              </div>

              {/* Simulated bar */}
              <div className="h-3 w-full bg-[#0d1117] rounded-sm overflow-hidden border border-[#30363d] mb-0.5">
                <div
                  className="h-full rounded-sm transition-all duration-500 ease-out"
                  style={{
                    width: `${Math.min(simulatedPct, 100)}%`,
                    backgroundColor: '#00d9ff',
                    opacity: 0.85,
                  }}
                />
              </div>

              {/* Testnet average bar */}
              <div className="h-2 w-full bg-[#0d1117] rounded-sm overflow-hidden border border-[#30363d]">
                <div
                  className="h-full rounded-sm transition-all duration-500 ease-out"
                  style={{
                    width: `${Math.min(averagePct, 100)}%`,
                    backgroundColor: '#8b949e',
                    opacity: 0.6,
                  }}
                />
              </div>

              <div className="h-[1px] bg-[#30363d] mt-3" />
            </div>
          );
        })}
      </div>

      {/* Cost Summary */}
      {cost_stroops !== undefined && (
        <div className="mt-6 pt-4 border-t-[4px] border-[#30363d]">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-[#c9d1d9]">
              <DollarSign size={16} className="text-[#00d9ff]" />
              <span className="font-bold text-sm">Total Simulated Cost</span>
            </div>
            <div className="text-right">
              <span className="font-bold text-[#00d9ff] text-base">
                {formatStroops(cost_stroops)}
              </span>
              <span className="text-xs text-[#8b949e] ml-2">
                ({formatCompact(cost_stroops)} stroops)
              </span>
            </div>
          </div>
        </div>
      )}

      <div className="mt-4 pt-4 border-t border-[#30363d]">
        <p className="text-[10px] text-[#8b949e] leading-tight">
          * Testnet averages are approximate and based on typical Soroban contract executions. Actual values may vary by contract complexity and network conditions.
        </p>
      </div>
    </div>
  );
};
