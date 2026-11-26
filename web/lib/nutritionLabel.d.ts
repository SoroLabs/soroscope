export interface NutritionResources {
  cpu_instructions: number;
  ram_bytes: number;
  ledger_read_bytes: number;
  ledger_write_bytes: number;
  transaction_size_bytes: number;
}

export type NutritionMetricKey = keyof NutritionResources;

export type NutritionSeverity = 'low' | 'moderate' | 'high' | 'over';

export interface NutritionMetricDefinition {
  key: NutritionMetricKey;
  label: string;
  unit: string;
  /** Primary metrics are rendered with heavier type, as on a real label. */
  bold: boolean;
}

export interface NutritionMetric extends NutritionMetricDefinition {
  /** Raw resource usage for this metric. */
  value: number;
  /** Per-transaction protocol ceiling used as the "daily value". */
  limit: number;
  /** Unclamped percent of the limit, so overruns stay visible. */
  percent: number;
  /** `percent` clamped to 0-100, for progress-bar widths. */
  barPercent: number;
  severity: NutritionSeverity;
}

export const SOROBAN_LIMITS: Readonly<NutritionResources>;
export const METRIC_DEFINITIONS: ReadonlyArray<NutritionMetricDefinition>;

export function dailyValuePercent(value: number, limit: number): number;
export function clampPercent(percent: number): number;
export function severityFor(percent: number): NutritionSeverity;
export function buildMetrics(
  resources: Partial<NutritionResources>,
  limits?: Partial<NutritionResources>,
): NutritionMetric[];
export function overallPercent(metrics: NutritionMetric[]): number;
export function formatValue(value: number): string;
export function formatPercent(percent: number): string;
