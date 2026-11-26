// Dedicated Web Worker for off-thread WASM bytecode decoding.
//
// Heavy WASM parsing/validation runs here so the main UI thread stays
// responsive (target 60fps) even for ~10MB uploads. The main thread
// communicates with this worker exclusively via postMessage.
//
// Protocol
//   main -> worker: { id, type: 'decode', buffer: ArrayBuffer }
//   worker -> main: { id, type: 'progress', phase, loaded, total }
//                   { id, type: 'result', result }
//                   { id, type: 'error', error }

const WASM_MAGIC = 0x6d736100; // '\0asm'
const WASM_VERSION = 0x1;

// Section ids per the WebAssembly binary format.
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
  12: 'dataCount',
};

function post(id, message) {
  self.postMessage({ id, ...message });
}

// Read an unsigned LEB128 integer from `bytes` starting at `offset`.
// Returns { value, next } or null when the buffer is truncated.
function readULEB128(bytes, offset) {
  let result = 0;
  let shift = 0;
  let pos = offset;
  while (pos < bytes.length) {
    const byte = bytes[pos++];
    result |= (byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      return { value: result >>> 0, next: pos };
    }
    shift += 7;
    if (shift > 35) {
      return null; // malformed / too long
    }
  }
  return null;
}

// Parse the module header and walk the section table. This is the heavy
// part that must not run on the UI thread.
function decodeWasm(bytes, id) {
  if (bytes.length < 8) {
    throw new Error('WASM binary is too small to contain a valid header.');
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const magic = view.getUint32(0, true);
  if (magic !== WASM_MAGIC) {
    throw new Error('Invalid WASM magic number: not a WebAssembly module.');
  }

  const version = view.getUint32(4, true);
  if (version !== WASM_VERSION) {
    throw new Error(`Unsupported WASM version: ${version}.`);
  }

  const sections = [];
  let offset = 8;
  let lastId = -1;

  while (offset < bytes.length) {
    const idRead = readULEB128(bytes, offset);
    if (!idRead) {
      throw new Error(`Truncated section id at byte ${offset}.`);
    }
    const sectionId = idRead.value;
    const sizeRead = readULEB128(bytes, idRead.next);
    if (!sizeRead) {
      throw new Error(`Truncated section size at byte ${idRead.next}.`);
    }
    const size = sizeRead.value;
    const bodyStart = sizeRead.next;
    const bodyEnd = bodyStart + size;

    if (bodyEnd > bytes.length) {
      throw new Error(
        `Section ${sectionId} declares ${size} bytes but only ${
          bytes.length - bodyStart
        } remain.`
      );
    }

    // Non-custom sections must appear in ascending id order.
    if (sectionId !== 0) {
      if (sectionId < lastId) {
        throw new Error(
          `Section ${sectionId} is out of order (after ${lastId}).`
        );
      }
      lastId = sectionId;
    }

    sections.push({
      id: sectionId,
      name: SECTION_NAMES[sectionId] || `unknown(${sectionId})`,
      size,
      offset: bodyStart,
    });

    offset = bodyEnd;
    post(id, {
      type: 'progress',
      phase: 'decode',
      loaded: offset,
      total: bytes.length,
    });
  }

  return {
    valid: true,
    version,
    byteLength: bytes.length,
    sections,
  };
}

self.onmessage = (event) => {
  const data = event.data || {};
  const { id, type } = data;

  if (type !== 'decode') {
    post(id, { type: 'error', error: `Unknown message type: ${type}` });
    return;
  }

  try {
    const bytes = new Uint8Array(data.buffer);
    const result = decodeWasm(bytes, id);
    post(id, { type: 'result', result });
  } catch (error) {
    post(id, {
      type: 'error',
      error: error && error.message ? error.message : String(error),
    });
  }
};
