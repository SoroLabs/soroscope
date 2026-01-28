export type SorobanType = 'address' | 'u32' | 'i128' | 'u128' | 'string' | 'symbol' | 'bool' | 'struct' | 'enum';

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

export interface InvocationResult {
  id: string;
  functionName: string;
  inputs: Record<string, any>;
  result?: any;
  error?: string;
  resourceCost?: {
    fee: string;
    instructions: string;
  };
  timestamp: number;
  success: boolean;
}

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
    fee: '0.01',
    instructions: (Math.random() * 2 + 0.5).toFixed(1) + 'M',
  };
}
