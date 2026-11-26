const WASM_MAGIC_NUMBER = 0x0061736d;
const WASM_SUPPORTED_VERSION = 1;
const MAX_WASM_FILE_SIZE_BYTES = 10 * 1024 * 1024;
const DEFAULT_UPLOAD_CHUNK_SIZE_BYTES = 128 * 1024;

/**
 * Splits [0, totalBytes) into contiguous {start, end} chunk ranges so a file
 * can be read incrementally instead of in one blocking pass.
 */
function computeChunkRanges(totalBytes, chunkSizeBytes) {
  if (chunkSizeBytes <= 0) {
    throw new Error('chunkSizeBytes must be greater than 0');
  }
  if (totalBytes <= 0) return [];

  const ranges = [];
  for (let start = 0; start < totalBytes; start += chunkSizeBytes) {
    ranges.push({ start, end: Math.min(start + chunkSizeBytes, totalBytes) });
  }
  return ranges;
}

function isWithinMaxFileSize(sizeBytes, maxBytes = MAX_WASM_FILE_SIZE_BYTES) {
  return sizeBytes <= maxBytes;
}

/**
 * Checks the WASM magic number (\0asm) and module version.
 * Throws with a user-facing message on the first failing check.
 */
function validateWasmBuffer(buffer) {
  if (buffer.byteLength < 8) {
    throw new Error('File is too small to be a valid WebAssembly module');
  }

  const view = new DataView(buffer);
  const magicNumber = view.getUint32(0, false);
  if (magicNumber !== WASM_MAGIC_NUMBER) {
    throw new Error('Invalid WASM magic number. File is not a valid WebAssembly module');
  }

  const version = view.getUint32(4, true);
  if (version !== WASM_SUPPORTED_VERSION) {
    throw new Error(
      `Unsupported WASM version: ${version}. Expected version ${WASM_SUPPORTED_VERSION}`
    );
  }
}

/**
 * Reads a File in sequential slices (default 128KB) instead of loading the
 * whole file in a single FileReader pass, so large uploads keep the main
 * thread free between chunks and can report real bytes-read progress.
 */
function readFileInChunks(file, onProgress, chunkSizeBytes = DEFAULT_UPLOAD_CHUNK_SIZE_BYTES) {
  const totalBytes = file.size;
  const ranges = computeChunkRanges(totalBytes, chunkSizeBytes);
  const result = new Uint8Array(totalBytes);

  if (ranges.length === 0) {
    return Promise.resolve(result.buffer);
  }

  return new Promise((resolve, reject) => {
    let index = 0;

    const readNextChunk = () => {
      const { start, end } = ranges[index];
      const reader = new FileReader();

      reader.onload = (event) => {
        const chunk = event.target?.result;
        result.set(new Uint8Array(chunk), start);
        onProgress?.(end, totalBytes);
        index += 1;

        if (index < ranges.length) {
          readNextChunk();
        } else {
          resolve(result.buffer);
        }
      };

      reader.onerror = () => {
        reject(reader.error ?? new Error('Unable to read the selected file'));
      };

      reader.readAsArrayBuffer(file.slice(start, end));
    };

    readNextChunk();
  });
}

module.exports = {
  WASM_MAGIC_NUMBER,
  WASM_SUPPORTED_VERSION,
  MAX_WASM_FILE_SIZE_BYTES,
  DEFAULT_UPLOAD_CHUNK_SIZE_BYTES,
  computeChunkRanges,
  isWithinMaxFileSize,
  validateWasmBuffer,
  readFileInChunks,
};
