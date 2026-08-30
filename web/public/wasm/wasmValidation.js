/**
 * Pure WASM binary decoding + validation helpers.
 *
 * Served from `public/` so the classic Web Worker in `wasmWorker.js` can pull it
 * in with `importScripts()` without any bundler involvement, while the app and
 * `node --test` load the very same file through `lib/wasmValidation.js`.
 *
 * Hence the UMD-style wrapper: one implementation, three consumers.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.SoroscopeWasmValidation = api;
  }
})(typeof self !== 'undefined' ? self : globalThis, function () {

  const WASM_MAGIC = [0x00, 0x61, 0x73, 0x6d];
  const WASM_VERSION = 1;

  const SECTION_NAMES = {
    0: 'custom',
    1: 'type',
    2: 'import',
    3: 'function',
    4: 'table',
    5: 'memory',
    6: 'global',
    7: 'export',
    8: 'start',
    9: 'element',
    10: 'code',
    11: 'data',
    12: 'data count',
  };

  const EXTERNAL_KINDS = {
    0: 'function',
    1: 'table',
    2: 'memory',
    3: 'global',
  };

  class WasmDecodeError extends Error {
    constructor(message, offset) {
      super(message);
      this.name = 'WasmDecodeError';
      this.offset = offset;
    }
  }

  function toBytes(input) {
    if (input instanceof Uint8Array) return input;
    if (input instanceof ArrayBuffer) return new Uint8Array(input);
    if (ArrayBuffer.isView(input)) {
      return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    }
    if (Array.isArray(input)) return Uint8Array.from(input);
    throw new WasmDecodeError('Unsupported input: expected ArrayBuffer or Uint8Array', 0);
  }

  /** Decode an unsigned LEB128 integer. Returns `{ value, next }`. */
  function readVarUint32(bytes, offset) {
    let result = 0;
    let shift = 0;
    let cursor = offset;

    while (true) {
      if (cursor >= bytes.length) {
        throw new WasmDecodeError('Unexpected end of buffer while reading LEB128 integer', cursor);
      }
      const byte = bytes[cursor];
      cursor += 1;
      result += (byte & 0x7f) * Math.pow(2, shift);
      if ((byte & 0x80) === 0) break;
      shift += 7;
      if (shift > 35) {
        throw new WasmDecodeError('LEB128 integer is too large for u32', cursor);
      }
    }

    return { value: result, next: cursor };
  }

  function readName(bytes, offset) {
    const { value: length, next } = readVarUint32(bytes, offset);
    const end = next + length;
    if (end > bytes.length) {
      throw new WasmDecodeError('Name length exceeds buffer size', next);
    }
    const slice = bytes.subarray(next, end);
    const decoded =
      typeof TextDecoder !== 'undefined'
        ? new TextDecoder('utf-8').decode(slice)
        : Array.from(slice, (b) => String.fromCharCode(b)).join('');
    return { value: decoded, next: end };
  }

  /** True when the buffer starts with the `\0asm` magic number. */
  function hasWasmMagic(input) {
    let bytes;
    try {
      bytes = toBytes(input);
    } catch {
      return false;
    }
    if (bytes.length < WASM_MAGIC.length) return false;
    return WASM_MAGIC.every((byte, index) => bytes[index] === byte);
  }

  function readVersion(bytes) {
    return bytes[4] | (bytes[5] << 8) | (bytes[6] << 16) | (bytes[7] << 24);
  }

  function parseExportSection(bytes, start, end) {
    const exports = [];
    let cursor = start;
    const { value: count, next } = readVarUint32(bytes, cursor);
    cursor = next;

    for (let i = 0; i < count && cursor < end; i += 1) {
      const name = readName(bytes, cursor);
      cursor = name.next;
      if (cursor >= end) {
        throw new WasmDecodeError('Truncated export entry', cursor);
      }
      const kindByte = bytes[cursor];
      cursor += 1;
      const index = readVarUint32(bytes, cursor);
      cursor = index.next;
      exports.push({
        name: name.value,
        kind: EXTERNAL_KINDS[kindByte] || `unknown(${kindByte})`,
        index: index.value,
      });
    }

    return exports;
  }

  function parseImportSection(bytes, start, end) {
    const imports = [];
    let cursor = start;
    const { value: count, next } = readVarUint32(bytes, cursor);
    cursor = next;

    for (let i = 0; i < count && cursor < end; i += 1) {
      const moduleName = readName(bytes, cursor);
      cursor = moduleName.next;
      const fieldName = readName(bytes, cursor);
      cursor = fieldName.next;
      if (cursor >= end) {
        throw new WasmDecodeError('Truncated import entry', cursor);
      }
      const kindByte = bytes[cursor];
      cursor += 1;

      imports.push({
        module: moduleName.value,
        name: fieldName.value,
        kind: EXTERNAL_KINDS[kindByte] || `unknown(${kindByte})`,
      });

      // Import descriptors have variable shapes; only the function form carries a
      // single type index we can cheaply skip. Anything else ends the scan.
      if (kindByte !== 0) break;
      const typeIndex = readVarUint32(bytes, cursor);
      cursor = typeIndex.next;
    }

    return imports;
  }

  /**
   * Walk every top-level section of a WASM module.
   *
   * This is the expensive part of validation for large contracts, which is why it
   * is offloaded to a Web Worker in the browser.
   */
  function parseWasmSections(input) {
    const bytes = toBytes(input);
    const sections = [];
    let cursor = 8; // skip magic + version

    while (cursor < bytes.length) {
      const id = bytes[cursor];
      cursor += 1;
      const { value: size, next } = readVarUint32(bytes, cursor);
      cursor = next;
      const end = cursor + size;

      if (end > bytes.length) {
        throw new WasmDecodeError(
          `Section "${SECTION_NAMES[id] || id}" claims ${size} bytes but only ${
            bytes.length - cursor
          } remain`,
          cursor,
        );
      }

      sections.push({
        id,
        name: SECTION_NAMES[id] || `unknown(${id})`,
        size,
        start: cursor,
        end,
      });

      cursor = end;
    }

    return sections;
  }

  /**
   * Decode and validate a WASM module, returning a structured report.
   *
   * Never throws: decode failures are reported through `errors` so the caller (or
   * the worker message channel) always receives a serialisable result.
   */
  function validateWasmModule(input, options = {}) {
    const started = Date.now();
    const maxBytes = typeof options.maxBytes === 'number' ? options.maxBytes : null;
    const errors = [];
    const warnings = [];

    let bytes;
    try {
      bytes = toBytes(input);
    } catch (error) {
    return {
      valid: false,
      byteLength: 0,
      version: null,
      sections: [],
      exports: [],
      imports: [],
      errors: [error.message],
      warnings,
      durationMs: Date.now() - started,
    };
  }

  const base = {
    byteLength: bytes.length,
    version: null,
    sections: [],
    exports: [],
    imports: [],
    warnings,
    durationMs: 0,
  };

  if (bytes.length === 0) {
    errors.push('File is empty');
  } else if (bytes.length < 8) {
    errors.push('File is too small to be a WASM module (needs at least 8 bytes)');
  } else if (!hasWasmMagic(bytes)) {
    errors.push('Missing WASM magic number (\\0asm) — file is not WebAssembly bytecode');
  }

  if (maxBytes !== null && bytes.length > maxBytes) {
    errors.push(`Module is ${bytes.length} bytes which exceeds the ${maxBytes} byte limit`);
  }

  if (errors.length > 0) {
    return { ...base, valid: false, errors, durationMs: Date.now() - started };
  }

  const version = readVersion(bytes);
  if (version !== WASM_VERSION) {
    errors.push(`Unsupported WASM binary version ${version} (expected ${WASM_VERSION})`);
    return { ...base, valid: false, version, errors, durationMs: Date.now() - started };
  }

  let sections = [];
  let exports = [];
  let imports = [];

  try {
    sections = parseWasmSections(bytes);
    const exportSection = sections.find((section) => section.id === 7);
    if (exportSection) {
      exports = parseExportSection(bytes, exportSection.start, exportSection.end);
    }
    const importSection = sections.find((section) => section.id === 2);
    if (importSection) {
      imports = parseImportSection(bytes, importSection.start, importSection.end);
    }
  } catch (error) {
    errors.push(error.message);
    return {
      ...base,
      valid: false,
      version,
      sections,
      exports,
      imports,
      errors,
      durationMs: Date.now() - started,
    };
  }

  if (exports.length === 0) {
    warnings.push('Module exports nothing — Soroban contracts must export their entry points');
  }
  if (!sections.some((section) => section.id === 10)) {
    warnings.push('Module has no code section');
  }

  return {
    valid: true,
    byteLength: bytes.length,
    version,
    sections: sections.map(({ id, name, size }) => ({ id, name, size })),
    exports,
    imports,
    errors,
    warnings,
    durationMs: Date.now() - started,
  };
}

/** Contract-facing summary: the exported functions a user can invoke. */
function extractContractFunctions(report) {
  if (!report || !Array.isArray(report.exports)) return [];
  return report.exports
    .filter((entry) => entry.kind === 'function')
    .map((entry) => entry.name)
    .filter((name) => name.length > 0 && !name.startsWith('_'))
    .sort((a, b) => a.localeCompare(b));
}

  return {
    WASM_MAGIC,
    WASM_VERSION,
    SECTION_NAMES,
    WasmDecodeError,
    hasWasmMagic,
    readVarUint32,
    parseWasmSections,
    validateWasmModule,
    extractContractFunctions,
  };
});
