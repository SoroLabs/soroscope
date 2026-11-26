import type { InvocationResult } from './sorobantypes';

export const DEFAULT_SHARE_BASE_URL: string;
export function isValidSimulationId(id: unknown): id is string;
export function resolveShareBaseUrl(override?: string | null): string;
export function buildShareUrl(simulationId: string, baseUrl?: string | null): string;
export function simulationRecordToResult(
  record: unknown,
  requestedId: string,
): { result: InvocationResult; contractId?: string };
