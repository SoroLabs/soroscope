export declare const OPCODE_TABLE: Record<number, string>;
export declare const OPCODE_TABLE_FC: Record<number, string>;
export declare const VALUE_TYPES: Record<number, string>;

export declare class WasmDisassemblyError extends Error {
  offset: number;
  constructor(message: string, offset: number);
}

export interface WasmInstruction {
  offset: number;
  size: number;
  opcode: number;
  mnemonic: string;
  immediate: string;
  operands: string[];
  text: string;
  unknown: boolean;
  funcIndex: number;
  funcName: string;
  indexInFunction: number;
  /** Stable `${funcIndex}:${indexInFunction}` key used by the cross-highlight map. */
  id: string;
}

export interface DisassembledFunction {
  index: number;
  name: string;
  nameSource: 'name' | 'export' | 'index';
  locals: string[];
  bodyStart: number;
  bodyEnd: number;
  truncated: boolean;
  instructions: WasmInstruction[];
}

export interface WasmSectionInfo {
  id: number;
  name: string;
  size: number;
  customName?: string;
}

export interface WasmImportInfo {
  module: string;
  name: string;
  kind: string;
}

export interface WasmExportInfo {
  name: string;
  kind: string;
  index: number;
}

export interface WasmDisassemblyResult {
  valid: boolean;
  byteLength: number;
  sections: WasmSectionInfo[];
  imports: WasmImportInfo[];
  exports: WasmExportInfo[];
  functions: DisassembledFunction[];
  instructions: WasmInstruction[];
  errors: string[];
}

export interface DisassembleOptions {
  /** Hard cap on how many functions are decoded. */
  maxFunctions?: number;
}

export interface RustFunctionRange {
  name: string;
  line: number;
  endLine: number;
}

export interface SourceLineMap {
  lineMap: Record<number, string[]>;
  matchedFunctions: string[];
  unmatchedFunctions: string[];
}

export declare function toBytes(
  input: ArrayBuffer | Uint8Array | number[],
): Uint8Array;

export declare function readVarUint32(
  bytes: Uint8Array,
  offset: number,
): { value: number; next: number };

export declare function readVarInt32(
  bytes: Uint8Array,
  offset: number,
): { value: number; next: number };

export declare function readVarInt64(
  bytes: Uint8Array,
  offset: number,
): { value: number; next: number };

export declare function decodeInstruction(
  bytes: Uint8Array,
  offset: number,
  context?: { localNames?: string[] | null; resolve?: (kind: string, index: number) => string | undefined },
): { instruction: Omit<WasmInstruction, 'funcIndex' | 'funcName' | 'indexInFunction' | 'id'>; next: number };

export declare function disassembleFunctionBody(
  bytes: Uint8Array,
  start: number,
  end: number,
  context?: { localNames?: string[] | null; resolve?: (kind: string, index: number) => string | undefined },
): { locals: string[]; instructions: Omit<WasmInstruction, 'funcIndex' | 'funcName' | 'indexInFunction' | 'id'>[] };

export declare function disassembleWasm(
  input: ArrayBuffer | Uint8Array | number[],
  options?: DisassembleOptions,
): WasmDisassemblyResult;

export declare function matchesInstruction(
  instruction: Pick<WasmInstruction, 'mnemonic' | 'text' | 'opcode'>,
  needle: string,
): boolean;

export declare function filterInstructions<T extends Pick<WasmInstruction, 'mnemonic' | 'text' | 'opcode'>>(
  instructions: T[],
  query: string,
): T[];

export declare function summarizeOpcodes(
  instructions: Array<Pick<WasmInstruction, 'mnemonic'>>,
): Array<{ mnemonic: string; count: number }>;

export declare function extractRustFunctionRanges(source: string): RustFunctionRange[];

export declare function normalizeFunctionName(name: string): string;

export declare function buildSourceLineMap(
  source: string,
  functions: Array<Pick<DisassembledFunction, 'name' | 'instructions'>>,
): SourceLineMap;
