export declare const WASM_MAGIC: number[];
export declare const WASM_VERSION: number;
export declare const SECTION_NAMES: Record<number, string>;

export declare class WasmDecodeError extends Error {
  offset: number;
  constructor(message: string, offset: number);
}

export interface WasmSectionSummary {
  id: number;
  name: string;
  size: number;
}

export interface WasmExportEntry {
  name: string;
  kind: string;
  index: number;
}

export interface WasmImportEntry {
  module: string;
  name: string;
  kind: string;
}

export interface WasmValidationReport {
  valid: boolean;
  byteLength: number;
  version: number | null;
  sections: WasmSectionSummary[];
  exports: WasmExportEntry[];
  imports: WasmImportEntry[];
  errors: string[];
  warnings: string[];
  durationMs: number;
}

export interface WasmValidationOptions {
  /** Reject modules larger than this many bytes. */
  maxBytes?: number;
}

export declare function hasWasmMagic(input: ArrayBuffer | Uint8Array | number[]): boolean;

export declare function readVarUint32(
  bytes: Uint8Array,
  offset: number,
): { value: number; next: number };

export declare function parseWasmSections(
  input: ArrayBuffer | Uint8Array,
): Array<WasmSectionSummary & { start: number; end: number }>;

export declare function validateWasmModule(
  input: ArrayBuffer | Uint8Array | number[],
  options?: WasmValidationOptions,
): WasmValidationReport;

export declare function extractContractFunctions(report: WasmValidationReport): string[];
