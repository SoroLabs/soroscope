export type GasGolfingSeverity = 'high' | 'medium' | 'low' | string;

export interface GasGolfingSuggestion {
  /** Unique within a report; used as the React row key (#1285). */
  id: string;
  title: string;
  severity: GasGolfingSeverity;
  description?: string;
  gas_saved_estimate?: number | null;
  recommendation?: string;
  code_example?: string;
  location?: string;
  pattern_type?: string;
}

export type GasGolfingSortKey = 'severity' | 'gas_saved_estimate';
export type SortDirection = 'asc' | 'desc';

export function sortGasGolfingSuggestions(
  suggestions: GasGolfingSuggestion[],
  sortKey: GasGolfingSortKey,
  direction: SortDirection,
): GasGolfingSuggestion[];


export function normalizeGasGolfingSuggestions(raw: unknown): GasGolfingSuggestion[];
