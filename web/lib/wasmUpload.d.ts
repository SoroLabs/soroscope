export const WASM_MAGIC_NUMBER: number;
export const WASM_SUPPORTED_VERSION: number;
export const MAX_WASM_FILE_SIZE_BYTES: number;
export const DEFAULT_UPLOAD_CHUNK_SIZE_BYTES: number;

export interface ChunkRange {
  start: number;
  end: number;
}

export function computeChunkRanges(totalBytes: number, chunkSizeBytes: number): ChunkRange[];

export function isWithinMaxFileSize(sizeBytes: number, maxBytes?: number): boolean;

export function validateWasmBuffer(buffer: ArrayBuffer): void;

export function readFileInChunks(
  file: File,
  onProgress?: (bytesRead: number, totalBytes: number) => void,
  chunkSizeBytes?: number,
): Promise<ArrayBuffer>;
