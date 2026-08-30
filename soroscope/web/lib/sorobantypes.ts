export type SorobanType = 'address' | 'u32' | 'i128' | 'u128' | 'string' | 'symbol' | 'bool' | 'struct' | 'enum';

export interface SorobanResources {
  cpu_instructions: number;
  ram_bytes: number;
  ledger_read_bytes: number;
  ledger_write_bytes: number;
  transaction_size_bytes: number;
}

export interface ContractFunction {
  name: string;
  inputs: ContractInput[];
  outputs?: SorobanType;
}

export interface ContractInput {
  name: string;
  type: SorobanType;
  description?: string;
  optional?: boolean;
}

export interface ResourceCost {
  fee?: string;
  cost_stroops?: number;
  cpu_instructions: number;
  ram_bytes: number;
  ledger_read_bytes: number;
  ledger_write_bytes: number;
  transaction_size_bytes: number;
  testnet_averages?: TestnetAverages;
}

export interface TestnetAverages {
  cpu_instructions: number;
  ram_bytes: number;
  ledger_read_bytes: number;
  ledger_write_bytes: number;
  transaction_size_bytes: number;
}

export interface InvocationResult {
  id: string;
  functionName: string;
  inputs: Record<string, any>;
  result?: any;
  error?: string;
  errorType?: string; // Error type from backend (e.g., BAD_REQUEST, INTERNAL_SERVER_ERROR)
  resourceCost?: {
    fee?: string;
    cpu_instructions: number;
    ram_bytes: number;
    ledger_read_bytes: number;
    ledger_write_bytes: number;
    transaction_size_bytes: number;
  };
  /** Primary `/analyze` response payload for the latest invocation. */
  analysisReport?: ResourceReport;
  /** Backward-compatible alias for older stored history entries. */
  resourceCost?: ResourceReport;
  resourceCost?: ResourceCost;
  callGraph?: CallGraph;
  callGraphMermaid?: string;
  stateSnapshot?: SimulationStateSnapshot;
  timestamp: number;
  success: boolean;
}

export interface CallNode {
  contract_id: string;
  function: string;
  children: CallNode[];
}

export interface CallGraph {
  root: CallNode;
}

export interface StateDependencyReport {
  key: string;
  source: 'Live' | 'Injected';
}

export interface TtlEntryApiReport {
  key: string;
  live_until_ledger: number;
  remaining_ledgers: number;
}

export interface ExtendTtlSuggestionApi {
  key: string;
  current_live_until_ledger: number;
  remaining_ledgers: number;
  extend_to_ledger: number;
  ledgers_to_extend_by: number;
  suggested_operation: string;
}

export interface TtlAnalysisApiReport {
  current_ledger: number;
  touched_entries: TtlEntryApiReport[];
  extend_ttl_suggestions: ExtendTtlSuggestionApi[];
}

export interface InsightEntry {
  severity: string;
  rule: string;
  message: string;
  suggested_fix: string;
}

export interface NutritionReport {
  efficiency_score: number;
  insights: InsightEntry[];
}

export interface SimulationStateSnapshot {
  ledger_entries: Record<string, string>;
  ttl_entries: Record<string, number>;
  latest_ledger: number;
}

export interface ResourceReport extends SorobanResources {
  cost_stroops: number;
  state_dependency: StateDependencyReport[] | null;
  ttl_analysis: TtlAnalysisApiReport | null;
  nutrition: NutritionReport;
  call_graph: CallGraph | null;
  call_graph_mermaid: string | null;
  state_snapshot: SimulationStateSnapshot | null;
  protocol_version: number;
}

export type AnalyzeResponse = ResourceReport;

// Mock contract functions for demo
export const MOCK_CONTRACT_FUNCTIONS: ContractFunction[] = [
  {
    name: 'transfer',
    inputs: [
      { name: 'from', type: 'address', description: 'Sender address' },
      { name: 'to', type: 'address', description: 'Recipient address' },
      { name: 'amount', type: 'u128', description: 'Amount to transfer' },
    ],
    outputs: 'bool',
  },
  {
    name: 'balance',
    inputs: [{ name: 'account', type: 'address', description: 'Account address' }],
    outputs: 'u128',
  },
  {
    name: 'mint',
    inputs: [
      { name: 'to', type: 'address', description: 'Recipient address' },
      { name: 'amount', type: 'u128', description: 'Amount to mint' },
    ],
    outputs: 'bool',
  },
  {
    name: 'symbol',
    inputs: [],
    outputs: 'string',
  },
  {
    name: 'decimals',
    inputs: [],
    outputs: 'u32',
  },
];

export function generateMockResult(functionName: string, inputs: Record<string, any>) {
  const results: Record<string, any> = {
    transfer: { success: true, transaction_hash: '0x' + Math.random().toString(16).slice(2) },
    balance: Math.floor(Math.random() * 1000000),
    mint: { success: true, amount_minted: inputs.amount },
    symbol: 'USDC',
    decimals: 6,
  };
  return results[functionName] || { success: true, message: 'Function executed' };
}

export function generateMockResourceCost() {
  return {
    fee: (Math.random() * 0.05).toFixed(5),
    cpu_instructions: Math.floor(Math.random() * 50_000_000) + 1_000_000,
    ram_bytes: Math.floor(Math.random() * 20 * 1024 * 1024) + 1024 * 1024,
    ledger_read_bytes: Math.floor(Math.random() * 10 * 1024),
    ledger_write_bytes: Math.floor(Math.random() * 5 * 1024),
    transaction_size_bytes: Math.floor(Math.random() * 2 * 1024),
  };
}
