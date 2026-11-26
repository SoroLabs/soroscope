import type { TransactionRecord, TransactionStatus } from './sorobantypes';

export interface TransactionFilter {
  status: TransactionStatus | 'all';
  functionName: string;
}

export const DEFAULT_TRANSACTION_FILTER: TransactionFilter = {
  status: 'all',
  functionName: '',
};

export function filterTransactions(
  transactions: TransactionRecord[],
  filter: TransactionFilter,
): TransactionRecord[] {
  const query = filter.functionName.trim().toLowerCase();

  return transactions.filter((tx) => {
    const matchesStatus = filter.status === 'all' || tx.status === filter.status;
    const matchesFunction = query.length === 0 || tx.functionName.toLowerCase().includes(query);
    return matchesStatus && matchesFunction;
  });
}
