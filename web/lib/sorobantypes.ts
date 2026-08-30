export type SorobanType =
  | 'address'
  | 'u32'
  | 'i128'
  | 'u128'
  | 'string'
  | 'symbol'
  | 'bool'
  | 'struct'
  | 'enum';

/** Typed map of contract function input values from the simulation form. */
export type SimulationInputs = Record<string, string | number | boolean>;

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

export interface ResourceCost extends SorobanResources {
  fee?: string;
  cost_stroops?: number;
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
  inputs: SimulationInputs;
  result?: unknown;
  error?: string;
  /** Error type from backend (e.g., BAD_REQUEST, INTERNAL_SERVER_ERROR) */
  errorType?: string;
  /** Primary `/analyze` response payload for the latest invocation. */
  analysisReport?: ResourceReport;
  /** Backward-compatible alias for older stored history entries. */
  resourceCost?: ResourceReport | ResourceCost;
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
  testnet_averages?: TestnetAverages;
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

export function generateMockResult(functionName: string, inputs: SimulationInputs): unknown {
  const results: Record<string, unknown> = {
    transfer: { success: true, transaction_hash: '0x' + Math.random().toString(16).slice(2) },
    balance: Math.floor(Math.random() * 1_000_000),
    mint: { success: true, amount_minted: inputs.amount },
    symbol: 'USDC',
    decimals: 6,
  };
  return results[functionName] ?? { success: true, message: 'Function executed' };
}

export type TransactionStatus = 'success' | 'failed' | 'pending';

export interface TransactionRecord {
  hash: string;
  functionName: string;
  status: TransactionStatus;
  timestamp: number;
  contractId: string;
  fee?: string;
}

export function generateMockTransactionRecord(overrides?: Partial<TransactionRecord>): TransactionRecord {
  return {
    hash: overrides?.hash ?? 'a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b',
    functionName: overrides?.functionName ?? 'transfer',
    status: overrides?.status ?? 'success',
    timestamp: overrides?.timestamp ?? Date.now(),
    contractId: overrides?.contractId ?? 'CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q',
    fee: overrides?.fee ?? '0.00123',
  };
}

export function generateMockTransactions(count: number): TransactionRecord[] {
  const statuses: TransactionStatus[] = ['success', 'failed', 'pending'];
  const functions = ['transfer', 'swap', 'mint', 'burn', 'deposit', 'withdraw', 'approve'];
  return Array.from({ length: count }, (_, i) => ({
    hash: `tx${String(i).padStart(3, '0')}${'a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b'.slice(5)}`,
    functionName: functions[i % functions.length],
    status: statuses[i % statuses.length],
    timestamp: Date.now() - i * 60000,
    contractId: 'CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q',
    fee: (Math.random() * 0.01).toFixed(5),
  }));
}

export function generateMockResourceCost(): ResourceCost {
  return {
    fee: (Math.random() * 0.05).toFixed(5),
    cpu_instructions: Math.floor(Math.random() * 50_000_000) + 1_000_000,
    ram_bytes: Math.floor(Math.random() * 20 * 1024 * 1024) + 1024 * 1024,
    ledger_read_bytes: Math.floor(Math.random() * 10 * 1024),
    ledger_write_bytes: Math.floor(Math.random() * 5 * 1024),
    transaction_size_bytes: Math.floor(Math.random() * 2 * 1024),
  };
}

export interface FeeBumpOption {
  label: string;
  multiplier: number;
  feeStroops: number;
  feeXlm: string;
  description: string;
}

export interface FeeEstimate {
  minResourceFeeStroops: number;
  classicFeeStroops: number;
  totalFeeStroops: number;
  totalFeeXlm: string;
  feeBumps: FeeBumpOption[];
  networkFees: {
    low: number;
    medium: number;
    high: number;
  };
  surgeMultiplier: number;
}

export type FeeBumpLevel = 'low' | 'medium' | 'high';
