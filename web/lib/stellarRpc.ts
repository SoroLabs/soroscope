/**
 * Stellar RPC client for fee estimation.
 *
 * Communicates with Soroban RPC endpoints to retrieve network fee stats and
 * compute transaction fee estimates from resource-cost data.
 */

import type { FeeBumpOption, FeeEstimate } from './sorobantypes';

const RPC_TIMEOUT_MS = 10_000;

export interface FeeStatsResponse {
  soroban_fee_charged: FeeStatDistribution;
  soroban_fee_rate: number;
  surge_multiplier: number;
  classic_fee_charged: FeeStatDistribution;
  classic_fee_rate: number;
  min_ledger_fee: number;
}

export interface FeeStatDistribution {
  max: number;
  min: number;
  mode: number;
  p10: number;
  p20: number;
  p30: number;
  p40: number;
  p50: number;
  p60: number;
  p70: number;
  p80: number;
  p90: number;
  p95: number;
  p99: number;
}

function parseStroops(value: string | number): number {
  if (typeof value === 'number') return value;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

export function stroopsToXlm(stroops: number): string {
  const xlm = stroops / 10_000_000;
  if (xlm === 0) return '0';
  if (xlm < 0.000001) return xlm.toFixed(9);
  if (xlm < 0.001) return xlm.toFixed(7);
  return xlm.toFixed(5);
}

export function formatStroops(stroops: number): string {
  const xlm = stroops / 10_000_000;
  return `${xlm.toFixed(7)} XLM`;
}

async function rpcCall<T>(
  rpcUrl: string,
  method: string,
  params?: unknown,
): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), RPC_TIMEOUT_MS);

  try {
    const response = await fetch(rpcUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method,
        params: params ?? {},
      }),
      signal: controller.signal,
    });

    if (!response.ok) {
      throw new Error(`RPC HTTP ${response.status}: ${response.statusText}`);
    }

    const data = await response.json();
    if (data.error) {
      throw new Error(`RPC error [${data.error.code}]: ${data.error.message}`);
    }

    return data.result as T;
  } finally {
    clearTimeout(timer);
  }
}

export async function fetchFeeStats(rpcUrl: string): Promise<FeeStatsResponse> {
  const result: Record<string, unknown> = await rpcCall(rpcUrl, 'getFeeStats');

  const parseDist = (prefix: string): FeeStatDistribution => {
    const v = (key: string) => parseStroops(result[`${prefix}_${key}`] as string | number);
    return {
      max: v('max'),
      min: v('min'),
      mode: v('mode'),
      p10: v('p10'),
      p20: v('p20'),
      p30: v('p30'),
      p40: v('p40'),
      p50: v('p50'),
      p60: v('p60'),
      p70: v('p70'),
      p80: v('p80'),
      p90: v('p90'),
      p95: v('p95'),
      p99: v('p99'),
    };
  };

  return {
    soroban_fee_charged: parseDist('soroban_fee_charged'),
    soroban_fee_rate: parseStroops(result.soroban_fee_rate as string | number),
    surge_multiplier: parseStroops(result.surge_multiplier as string | number),
    classic_fee_charged: parseDist('classic_fee_charged'),
    classic_fee_rate: parseStroops(result.classic_fee_rate as string | number),
    min_ledger_fee: parseStroops(result.last_ledger_base_fee as string | number),
  };
}

export enum FeeBumpLevel {
  LOW = 'low',
  MEDIUM = 'medium',
  HIGH = 'high',
}

const BUMP_CONFIG: Record<FeeBumpLevel, { label: string; multiplier: number; description: string }> = {
  [FeeBumpLevel.LOW]: {
    label: 'Low (Min)',
    multiplier: 1.0,
    description: 'Minimum fee, slower confirmation',
  },
  [FeeBumpLevel.MEDIUM]: {
    label: 'Medium',
    multiplier: 1.5,
    description: 'Balanced speed and cost',
  },
  [FeeBumpLevel.HIGH]: {
    label: 'High',
    multiplier: 2.0,
    description: 'Priority processing',
  },
};

export function estimateFees(
  resourceCostStroops: number,
  feeStats?: FeeStatsResponse | null,
): FeeEstimate {
  const baseClassicFee = feeStats?.min_ledger_fee ?? 100;
  const surge = feeStats?.surge_multiplier ?? 1;
  const sorobanRate = feeStats?.soroban_fee_rate ?? 1;

  const minResourceFeeStroops = resourceCostStroops;
  const classicFeeStroops = baseClassicFee;
  const baseTotal = minResourceFeeStroops + classicFeeStroops;

  const surgeMultiplier = surge;

  const feeBumps: FeeBumpOption[] = Object.values(BUMP_CONFIG).map((cfg) => {
    const feeStroops = Math.ceil(baseTotal * cfg.multiplier * sorobanRate);
    return {
      label: cfg.label,
      multiplier: cfg.multiplier,
      feeStroops,
      feeXlm: stroopsToXlm(feeStroops),
      description: cfg.description,
    };
  });

  return {
    minResourceFeeStroops,
    classicFeeStroops,
    totalFeeStroops: baseTotal,
    totalFeeXlm: stroopsToXlm(baseTotal),
    feeBumps,
    networkFees: {
      low: feeBumps[0].feeStroops,
      medium: feeBumps[1].feeStroops,
      high: feeBumps[2].feeStroops,
    },
    surgeMultiplier,
  };
}
