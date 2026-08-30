export type GasGolfingSeverity = 'high' | 'medium' | 'low' | string;

export interface GasGolfingSuggestion {
  title: string;
  severity: GasGolfingSeverity;
  description?: string;
  gas_saved_estimate?: number | null;
}

export type GasGolfingSortKey = 'severity' | 'gas_saved_estimate';
export type SortDirection = 'asc' | 'desc';

export function sortGasGolfingSuggestions(
  suggestions: GasGolfingSuggestion[],
  sortKey: GasGolfingSortKey,
  direction: SortDirection,
): GasGolfingSuggestion[];

