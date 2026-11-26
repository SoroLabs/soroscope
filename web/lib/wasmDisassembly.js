/**
 * WASM bytecode decompiler primitives for the Disassembly tab.
 *
 * Pure, dependency-free and CommonJS-flavoured so the same module powers the
 * React viewer (`WasmDisassemblyViewer.tsx`) and `node --test` unit tests.
 *
 * Scope: structural decode of the code section (locals + instruction stream),
 * symbolisation via the `name` custom section with an export-section fallback,
 * opcode filtering, and the Rust-source -> opcode cross-highlight map.
 */

const { parseWasmSections, readVarUint32 } = require('./wasmValidation.js');

class WasmDisassemblyError extends Error {
  constructor(message, offset) {
    super(message);
    this.name = 'WasmDisassemblyError';
    this.offset = offset;
  }
}

/**
 * `code:mnemonic:immediate` triples. `immediate` drives how the decoder consumes
 * the bytes that follow the opcode:
 *   none | blocktype | localidx | globalidx | funcidx | tableidx | labelidx
 *   brtable | memarg | i32 | i64 | f32 | f64 | reftype | valtypes | memidx
 *   dataidx | elemidx | typeidx | memcopy | tablecopy | sat_trunc
 */
const OPCODE_TABLE = {
  0x00: 'unreachable:none',
  0x01: 'nop:none',
  0x02: 'block:blocktype',
  0x03: 'loop:blocktype',
  0x04: 'if:blocktype',
  0x05: 'else:none',
  0x0b: 'end:none',
  0x0c: 'br:labelidx',
  0x0d: 'br_if:labelidx',
  0x0e: 'br_table:brtable',
  0x0f: 'return:none',
  0x10: 'call:funcidx',
  0x11: 'call_indirect:typeidx',
  0x1a: 'drop:none',
  0x1b: 'select:none',
  0x1c: 'select:valtypes',
  0x20: 'local.get:localidx',
  0x21: 'local.set:localidx',
  0x22: 'local.tee:localidx',
  0x23: 'global.get:globalidx',
  0x24: 'global.set:globalidx',
  0x25: 'table.get:tableidx',
  0x26: 'table.set:tableidx',
  0x28: 'i32.load:memarg',
  0x29: 'i64.load:memarg',
  0x2a: 'f32.load:memarg',
  0x2b: 'f64.load:memarg',
  0x2c: 'i32.load8_s:memarg',
  0x2d: 'i32.load8_u:memarg',
  0x2e: 'i32.load16_s:memarg',
  0x2f: 'i32.load16_u:memarg',
  0x30: 'i64.load8_s:memarg',
  0x31: 'i64.load8_u:memarg',
  0x32: 'i64.load16_s:memarg',
  0x33: 'i64.load16_u:memarg',
  0x34: 'i64.load32_s:memarg',
  0x35: 'i64.load32_u:memarg',
  0x36: 'i32.store:memarg',
  0x37: 'i64.store:memarg',
  0x38: 'f32.store:memarg',
  0x39: 'f64.store:memarg',
  0x3a: 'i32.store8:memarg',
  0x3b: 'i32.store16:memarg',
  0x3c: 'i64.store8:memarg',
  0x3d: 'i64.store16:memarg',
  0x3e: 'i64.store32:memarg',
  0x3f: 'memory.size:memidx',
  0x40: 'memory.grow:memidx',
  0x41: 'i32.const:i32',
  0x42: 'i64.const:i64',
  0x43: 'f32.const:f32',
  0x44: 'f64.const:f64',
  0x45: 'i32.eqz:none',
  0x46: 'i32.eq:none',
  0x47: 'i32.ne:none',
  0x48: 'i32.lt_s:none',
  0x49: 'i32.lt_u:none',
  0x4a: 'i32.gt_s:none',
  0x4b: 'i32.gt_u:none',
  0x4c: 'i32.le_s:none',
  0x4d: 'i32.le_u:none',
  0x4e: 'i32.ge_s:none',
  0x4f: 'i32.ge_u:none',
  0x50: 'i64.eqz:none',
  0x51: 'i64.eq:none',
  0x52: 'i64.ne:none',
  0x53: 'i64.lt_s:none',
  0x54: 'i64.lt_u:none',
  0x55: 'i64.gt_s:none',
  0x56: 'i64.gt_u:none',
  0x57: 'i64.le_s:none',
  0x58: 'i64.le_u:none',
  0x59: 'i64.ge_s:none',
  0x5a: 'i64.ge_u:none',
  0x5b: 'f32.eq:none',
  0x5c: 'f32.ne:none',
  0x5d: 'f32.lt:none',
  0x5e: 'f32.gt:none',
  0x5f: 'f32.le:none',
  0x60: 'f32.ge:none',
  0x61: 'f64.eq:none',
  0x62: 'f64.ne:none',
  0x63: 'f64.lt:none',
  0x64: 'f64.gt:none',
  0x65: 'f64.le:none',
  0x66: 'f64.ge:none',
  0x67: 'i32.clz:none',
  0x68: 'i32.ctz:none',
  0x69: 'i32.popcnt:none',
  0x6a: 'i32.add:none',
  0x6b: 'i32.sub:none',
  0x6c: 'i32.mul:none',
  0x6d: 'i32.div_s:none',
  0x6e: 'i32.div_u:none',
  0x6f: 'i32.rem_s:none',
  0x70: 'i32.rem_u:none',
  0x71: 'i32.and:none',
  0x72: 'i32.or:none',
  0x73: 'i32.xor:none',
  0x74: 'i32.shl:none',
  0x75: 'i32.shr_s:none',
  0x76: 'i32.shr_u:none',
  0x77: 'i32.rotl:none',
  0x78: 'i32.rotr:none',
  0x79: 'i64.clz:none',
  0x7a: 'i64.ctz:none',
  0x7b: 'i64.popcnt:none',
  0x7c: 'i64.add:none',
  0x7d: 'i64.sub:none',
  0x7e: 'i64.mul:none',
  0x7f: 'i64.div_s:none',
  0x80: 'i64.div_u:none',
  0x81: 'i64.rem_s:none',
  0x82: 'i64.rem_u:none',
  0x83: 'i64.and:none',
  0x84: 'i64.or:none',
  0x85: 'i64.xor:none',
  0x86: 'i64.shl:none',
  0x87: 'i64.shr_s:none',
  0x88: 'i64.shr_u:none',
  0x89: 'i64.rotl:none',
  0x8a: 'i64.rotr:none',
  0x8b: 'f32.abs:none',
  0x8c: 'f32.neg:none',
  0x8d: 'f32.ceil:none',
  0x8e: 'f32.floor:none',
  0x8f: 'f32.trunc:none',
  0x90: 'f32.nearest:none',
  0x91: 'f32.sqrt:none',
  0x92: 'f32.add:none',
  0x93: 'f32.sub:none',
  0x94: 'f32.mul:none',
  0x95: 'f32.div:none',
  0x96: 'f32.min:none',
  0x97: 'f32.max:none',
  0x98: 'f32.copysign:none',
  0x99: 'f64.abs:none',
  0x9a: 'f64.neg:none',
  0x9b: 'f64.ceil:none',
  0x9c: 'f64.floor:none',
  0x9d: 'f64.trunc:none',
  0x9e: 'f64.nearest:none',
  0x9f: 'f64.sqrt:none',
  0xa0: 'f64.add:none',
  0xa1: 'f64.sub:none',
  0xa2: 'f64.mul:none',
  0xa3: 'f64.div:none',
  0xa4: 'f64.min:none',
  0xa5: 'f64.max:none',
  0xa6: 'f64.copysign:none',
  0xa7: 'i32.wrap_i64:none',
  0xa8: 'i32.trunc_f32_s:none',
  0xa9: 'i32.trunc_f32_u:none',
  0xaa: 'i32.trunc_f64_s:none',
  0xab: 'i32.trunc_f64_u:none',
  0xac: 'i64.extend_i32_s:none',
  0xad: 'i64.extend_i32_u:none',
  0xae: 'i64.trunc_f32_s:none',
  0xaf: 'i64.trunc_f32_u:none',
  0xb0: 'i64.trunc_f64_s:none',
  0xb1: 'i64.trunc_f64_u:none',
  0xb2: 'f32.convert_i32_s:none',
  0xb3: 'f32.convert_i32_u:none',
  0xb4: 'f32.convert_i64_s:none',
  0xb5: 'f32.convert_i64_u:none',
  0xb6: 'f32.demote_f64:none',
  0xb7: 'f64.convert_i32_s:none',
  0xb8: 'f64.convert_i32_u:none',
  0xb9: 'f64.convert_i64_s:none',
  0xba: 'f64.convert_i64_u:none',
  0xbb: 'f64.promote_f32:none',
  0xbc: 'i32.reinterpret_f32:none',
  0xbd: 'i64.reinterpret_f64:none',
  0xbe: 'f32.reinterpret_i32:none',
  0xbf: 'f64.reinterpret_i64:none',
  0xc0: 'i32.extend8_s:none',
  0xc1: 'i32.extend16_s:none',
  0xc2: 'i64.extend8_s:none',
  0xc3: 'i64.extend16_s:none',
  0xc4: 'i64.extend32_s:none',
  0xd0: 'ref.null:reftype',
  0xd1: 'ref.is_null:none',
  0xd2: 'ref.func:funcidx',
  0xfc: 'prefix:none',
};

/** `0xfc` prefixed opcodes (saturating truncation + bulk memory + table bulk). */
const OPCODE_TABLE_FC = {
  0: 'i32.trunc_sat_f32_s:none',
  1: 'i32.trunc_sat_f32_u:none',
  2: 'i32.trunc_sat_f64_s:none',
  3: 'i32.trunc_sat_f64_u:none',
  4: 'i64.trunc_sat_f32_s:none',
  5: 'i64.trunc_sat_f32_u:none',
  6: 'i64.trunc_sat_f64_s:none',
  7: 'i64.trunc_sat_f64_u:none',
  8: 'memory.init:dataidx',
  9: 'data.drop:dataidx',
  10: 'memory.copy:memcopy',
  11: 'memory.fill:memidx',
  12: 'table.init:elemidx',
  13: 'elem.drop:elemidx',
  14: 'table.copy:tablecopy',
  15: 'table.grow:tableidx',
  16: 'table.size:tableidx',
  17: 'table.fill:tableidx',
};

const VALUE_TYPES = {
  0x7f: 'i32',
  0x7e: 'i64',
  0x7d: 'f32',
  0x7c: 'f64',
  0x7b: 'v128',
  0x70: 'funcref',
  0x6f: 'externref',
};

const EXTERNAL_KIND_NAMES = {
  0: 'function',
  1: 'table',
  2: 'memory',
  3: 'global',
};

function toBytes(input) {
  if (input instanceof Uint8Array) return input;
  if (input instanceof ArrayBuffer) return new Uint8Array(input);
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }
  if (Array.isArray(input)) return Uint8Array.from(input);
  throw new WasmDisassemblyError('Unsupported input: expected ArrayBuffer or Uint8Array', 0);
}

/** Decode a signed LEB128 integer of `bits` width. Returns `{ value, next }`. */
function readVarInt(bytes, offset, bits) {
  let result = 0;
  let shift = 0;
  let cursor = offset;
  let byte = 0;

  do {
    if (cursor >= bytes.length) {
      throw new WasmDisassemblyError('Unexpected end of buffer in signed LEB128 integer', cursor);
    }
    byte = bytes[cursor];
    cursor += 1;
    result += (byte & 0x7f) * Math.pow(2, shift);
    shift += 7;
  } while (byte & 0x80);

  if (shift < bits && byte & 0x40) {
    result -= Math.pow(2, shift);
  }

  return { value: result, next: cursor };
}

function readVarInt32(bytes, offset) {
  return readVarInt(bytes, offset, 32);
}

function readVarInt64(bytes, offset) {
  return readVarInt(bytes, offset, 64);
}

function readVarS33(bytes, offset) {
  return readVarInt(bytes, offset, 33);
}

function readName(bytes, offset) {
  const { value: length, next } = readVarUint32(bytes, offset);
  const end = next + length;
  if (end > bytes.length) {
    throw new WasmDisassemblyError('Name length exceeds buffer size', next);
  }
  const slice = bytes.subarray(next, end);
  const decoded =
    typeof TextDecoder !== 'undefined'
      ? new TextDecoder('utf-8').decode(slice)
      : Array.from(slice, (b) => String.fromCharCode(b)).join('');
  return { value: decoded, next: end };
}

function readF32(bytes, offset) {
  if (offset + 4 > bytes.length) {
    throw new WasmDisassemblyError('Unexpected end of buffer reading f32', offset);
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset + offset, 4);
  return { value: view.getFloat32(0, true), next: offset + 4 };
}

function readF64(bytes, offset) {
  if (offset + 8 > bytes.length) {
    throw new WasmDisassemblyError('Unexpected end of buffer reading f64', offset);
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset + offset, 8);
  return { value: view.getFloat64(0, true), next: offset + 8 };
}

/** Skip a `limits` record (flags + optional max). */
function skipLimits(bytes, offset) {
  let cursor = offset;
  const flags = readVarUint32(bytes, cursor);
  cursor = flags.next;
  const min = readVarUint32(bytes, cursor);
  cursor = min.next;
  if (flags.value & 0x01) {
    const max = readVarUint32(bytes, cursor);
    cursor = max.next;
  }
  return cursor;
}

/**
 * Parse the import section fully, because imported functions occupy the first
 * slots of the function index space and every `call` operand depends on it.
 */
function parseImports(bytes, start, end) {
  const imports = [];
  let cursor = start;
  const count = readVarUint32(bytes, cursor);
  cursor = count.next;

  for (let i = 0; i < count.value && cursor < end; i += 1) {
    const moduleName = readName(bytes, cursor);
    cursor = moduleName.next;
    const fieldName = readName(bytes, cursor);
    cursor = fieldName.next;
    if (cursor >= end) {
      throw new WasmDisassemblyError('Truncated import entry', cursor);
    }
    const kindByte = bytes[cursor];
    cursor += 1;
    const entry = {
      module: moduleName.value,
      name: fieldName.value,
      kind: EXTERNAL_KIND_NAMES[kindByte] || `unknown(${kindByte})`,
    };

    if (kindByte === 0) {
      const typeIndex = readVarUint32(bytes, cursor);
      cursor = typeIndex.next;
    } else if (kindByte === 1) {
      cursor = skipLimits(bytes, cursor + 1); // reftype byte, then limits
    } else if (kindByte === 2) {
      cursor = skipLimits(bytes, cursor);
    } else if (kindByte === 3) {
      cursor += 2; // valtype + mutability
    } else {
      break;
    }

    imports.push(entry);
  }

  return imports;
}

function parseExports(bytes, start, end) {
  const exports = [];
  let cursor = start;
  const count = readVarUint32(bytes, cursor);
  cursor = count.next;

  for (let i = 0; i < count.value && cursor < end; i += 1) {
    const name = readName(bytes, cursor);
    cursor = name.next;
    if (cursor >= end) break;
    const kindByte = bytes[cursor];
    cursor += 1;
    const index = readVarUint32(bytes, cursor);
    cursor = index.next;
    exports.push({
      name: name.value,
      kind: EXTERNAL_KIND_NAMES[kindByte] || `unknown(${kindByte})`,
      index: index.value,
    });
  }

  return exports;
}

/**
 * Read the `name` custom section's function-name subsection, which is what
 * `rustc` emits (`liquidity_pool::swap`) and is far friendlier than `func[7]`.
 */
function parseNameSectionFunctionNames(bytes, start, end) {
  const names = new Map();
  let cursor = start;

  while (cursor < end) {
    const subsectionId = bytes[cursor];
    cursor += 1;
    const size = readVarUint32(bytes, cursor);
    cursor = size.next;
    const subEnd = Math.min(cursor + size.value, end);

    if (subsectionId === 1) {
      const count = readVarUint32(bytes, cursor);
      let entry = count.next;
      for (let i = 0; i < count.value && entry < subEnd; i += 1) {
        const index = readVarUint32(bytes, entry);
        entry = index.next;
        const name = readName(bytes, entry);
        entry = name.next;
        names.set(index.value, name.value);
      }
    }

    cursor = subEnd;
  }

  return names;
}

function findCustomSection(sections, name) {
  return sections.find((section) => section.id === 0 && section.customName === name) || null;
}

function annotateCustomNames(bytes, sections) {
  for (const section of sections) {
    if (section.id !== 0) continue;
    let cursor = section.start;
    let name = '';
    try {
      const parsed = readName(bytes, cursor);
      name = parsed.value;
      cursor = parsed.next;
    } catch {
      continue;
    }
    section.customName = name;
    section.payloadStart = cursor;
  }
}

/** Decode a single instruction at `offset`. Returns `{ instruction, next }`. */
function decodeInstruction(bytes, offset, context) {
  if (offset >= bytes.length) {
    throw new WasmDisassemblyError('Unexpected end of code section', offset);
  }

  const opcodeByte = bytes[offset];
  const entry = OPCODE_TABLE[opcodeByte];

  if (!entry) {
    return {
      instruction: {
        offset,
        size: 1,
        opcode: opcodeByte,
        mnemonic: `unknown.0x${opcodeByte.toString(16).padStart(2, '0')}`,
        immediate: 'unknown',
        operands: [],
        text: `unknown.0x${opcodeByte.toString(16).padStart(2, '0')}`,
        unknown: true,
      },
      next: offset + 1,
    };
  }

  const [mnemonic, immediate] = entry.split(':');
  let cursor = offset + 1;
  const operands = [];

  if (opcodeByte === 0xfc) {
    const sub = readVarUint32(bytes, cursor);
    cursor = sub.next;
    const subEntry = OPCODE_TABLE_FC[sub.value];
    if (!subEntry) {
      const label = `unknown.0xfc.0x${sub.value.toString(16)}`;
      return {
        instruction: {
          offset,
          size: cursor - offset,
          opcode: 0xfc00 + sub.value,
          mnemonic: label,
          immediate: 'unknown',
          operands: [],
          text: label,
          unknown: true,
        },
        next: cursor,
      };
    }
    const [subMnemonic, subImmediate] = subEntry.split(':');
    return decodeImmediate(bytes, offset, cursor, 0xfc00 + sub.value, subMnemonic, subImmediate, context);
  }

  return decodeImmediate(bytes, offset, cursor, opcodeByte, mnemonic, immediate, context);
}

function readBlockType(bytes, offset) {
  const byte = bytes[offset];
  if (byte === 0x40) return { text: 'void', next: offset + 1 };
  if (VALUE_TYPES[byte]) return { text: VALUE_TYPES[byte], next: offset + 1 };
  const signed = readVarS33(bytes, offset);
  if (signed.value >= 0) return { text: `type[${signed.value}]`, next: signed.next };
  return { text: `valtype.${signed.value}`, next: signed.next };
}

function readMemArg(bytes, offset) {
  let cursor = offset;
  const align = readVarUint32(bytes, cursor);
  cursor = align.next;
  const memOffset = readVarUint32(bytes, cursor);
  cursor = memOffset.next;
  // Multi-memory encodes the memory index in bit 6 of the alignment field.
  if (align.value & 0x40) {
    const memIndex = readVarUint32(bytes, cursor);
    cursor = memIndex.next;
    return {
      text: `offset=${memOffset.value} align=${align.value & 0x3f} mem=${memIndex.value}`,
      next: cursor,
    };
  }
  return { text: `offset=${memOffset.value} align=${align.value}`, next: cursor };
}

function decodeImmediate(bytes, offset, cursor, opcodeByte, mnemonic, immediate, context) {
  const operands = [];
  const resolve = context && typeof context.resolve === 'function' ? context.resolve : () => undefined;

  switch (immediate) {
    case 'none':
      break;

    case 'blocktype': {
      const parsed = readBlockType(bytes, cursor);
      operands.push(parsed.text);
      cursor = parsed.next;
      break;
    }

    case 'localidx': {
      const index = readVarUint32(bytes, cursor);
      cursor = index.next;
      const local = context && context.localNames ? context.localNames[index.value] : undefined;
      operands.push(local ? `${index.value} (${local})` : String(index.value));
      break;
    }

    case 'globalidx': {
      const index = readVarUint32(bytes, cursor);
      cursor = index.next;
      operands.push(String(index.value));
      break;
    }

    case 'funcidx': {
      const index = readVarUint32(bytes, cursor);
      cursor = index.next;
      const name = resolve('function', index.value);
      operands.push(name ? `${index.value} (${name})` : String(index.value));
      break;
    }

    case 'tableidx': {
      const index = readVarUint32(bytes, cursor);
      cursor = index.next;
      operands.push(String(index.value));
      break;
    }

    case 'typeidx': {
      const type = readVarUint32(bytes, cursor);
      cursor = type.next;
      const table = readVarUint32(bytes, cursor);
      cursor = table.next;
      operands.push(String(type.value), String(table.value));
      break;
    }

    case 'labelidx': {
      const depth = readVarUint32(bytes, cursor);
      cursor = depth.next;
      operands.push(String(depth.value));
      break;
    }

    case 'brtable': {
      const count = readVarUint32(bytes, cursor);
      cursor = count.next;
      const targets = [];
      for (let i = 0; i < count.value && cursor < bytes.length; i += 1) {
        const target = readVarUint32(bytes, cursor);
        cursor = target.next;
        targets.push(String(target.value));
      }
      const fallback = readVarUint32(bytes, cursor);
      cursor = fallback.next;
      targets.push(String(fallback.value));
      operands.push(targets.join(' '));
      break;
    }

    case 'memarg': {
      const parsed = readMemArg(bytes, cursor);
      operands.push(parsed.text);
      cursor = parsed.next;
      break;
    }

    case 'memidx': {
      const index = readVarUint32(bytes, cursor);
      cursor = index.next;
      operands.push(String(index.value));
      break;
    }

    case 'memcopy': {
      const destination = readVarUint32(bytes, cursor);
      cursor = destination.next;
      const source = readVarUint32(bytes, cursor);
      cursor = source.next;
      operands.push(String(destination.value), String(source.value));
      break;
    }

    case 'tablecopy': {
      const destination = readVarUint32(bytes, cursor);
      cursor = destination.next;
      const source = readVarUint32(bytes, cursor);
      cursor = source.next;
      operands.push(String(destination.value), String(source.value));
      break;
    }

    case 'dataidx':
    case 'elemidx': {
      const index = readVarUint32(bytes, cursor);
      cursor = index.next;
      operands.push(String(index.value));
      break;
    }

    case 'reftype': {
      const signed = readVarS33(bytes, cursor);
      cursor = signed.next;
      operands.push(signed.value < 0 ? refTypeName(signed.value) : `type[${signed.value}]`);
      break;
    }

    case 'valtypes': {
      const count = readVarUint32(bytes, cursor);
      cursor = count.next;
      const types = [];
      for (let i = 0; i < count.value && cursor < bytes.length; i += 1) {
        types.push(VALUE_TYPES[bytes[cursor]] || `0x${bytes[cursor].toString(16)}`);
        cursor += 1;
      }
      operands.push(types.join(' '));
      break;
    }

    case 'i32': {
      const signed = readVarInt32(bytes, cursor);
      cursor = signed.next;
      operands.push(String(signed.value));
      break;
    }

    case 'i64': {
      const signed = readVarInt64(bytes, cursor);
      cursor = signed.next;
      // Beyond 2^53 the exact integer is lost, so keep the raw LEB suffix too.
      operands.push(Number.isSafeInteger(signed.value) ? String(signed.value) : `${signed.value}`);
      break;
    }

    case 'f32': {
      const parsed = readF32(bytes, cursor);
      cursor = parsed.next;
      operands.push(formatFloat(parsed.value));
      break;
    }

    case 'f64': {
      const parsed = readF64(bytes, cursor);
      cursor = parsed.next;
      operands.push(formatFloat(parsed.value));
      break;
    }

    default:
      break;
  }

  const text = operands.length > 0 ? `${mnemonic} ${operands.join(' ')}` : mnemonic;
  return {
    instruction: {
      offset,
      size: cursor - offset,
      opcode: opcodeByte,
      mnemonic,
      immediate,
      operands,
      text,
      unknown: false,
    },
    next: cursor,
  };
}

function refTypeName(value) {
  const map = { '-16': 'funcref', '-17': 'externref' };
  return map[String(value)] || `reftype.${value}`;
}

function formatFloat(value) {
  if (!Number.isFinite(value)) return String(value);
  return String(value);
}

/** Decode a whole function body (`locals` + instruction stream) to instructions. */
function disassembleFunctionBody(bytes, start, end, context) {
  let cursor = start;
  const localDeclCount = readVarUint32(bytes, cursor);
  cursor = localDeclCount.next;

  const locals = [];
  for (let i = 0; i < localDeclCount.value && cursor < end; i += 1) {
    const repeat = readVarUint32(bytes, cursor);
    cursor = repeat.next;
    const type = VALUE_TYPES[bytes[cursor]] || `0x${(bytes[cursor] ?? 0).toString(16)}`;
    cursor += 1;
    for (let n = 0; n < repeat.value; n += 1) {
      locals.push(type);
    }
  }

  const instructions = [];
  while (cursor < end) {
    let decoded;
    try {
      decoded = decodeInstruction(bytes, cursor, context);
    } catch (error) {
      instructions.push({
        offset: cursor,
        size: end - cursor,
        opcode: -1,
        mnemonic: 'invalid',
        immediate: 'invalid',
        operands: [],
        text: `; ${error.message}`,
        unknown: true,
      });
      cursor = end;
      break;
    }
    // Defensive guard: a bad immediate must never spin the decoder forever.
    if (decoded.next <= cursor) {
      cursor = end;
      break;
    }
    instructions.push(decoded.instruction);
    cursor = decoded.next;
  }

  return { locals, instructions };
}

/**
 * Disassemble every function in a WASM module.
 *
 * Never throws: decode problems are surfaced through `errors` plus per-function
 * `truncated` flags so the viewer can still render partial output.
 */
function disassembleWasm(input, options = {}) {
  const maxFunctions = typeof options.maxFunctions === 'number' ? options.maxFunctions : 5000;

  let bytes;
  try {
    bytes = toBytes(input);
  } catch (error) {
    return {
      valid: false,
      byteLength: 0,
      sections: [],
      imports: [],
      exports: [],
      functions: [],
      instructions: [],
      errors: [error.message],
    };
  }

  const base = {
    valid: false,
    byteLength: bytes.length,
    sections: [],
    imports: [],
    exports: [],
    functions: [],
    instructions: [],
    errors: [],
  };

  if (bytes.length < 8) {
    return { ...base, errors: ['Module is too small to be a WASM module'] };
  }

  let sections;
  try {
    sections = parseWasmSections(bytes);
    annotateCustomNames(bytes, sections);
  } catch (error) {
    return { ...base, errors: [error.message] };
  }

  const errors = [];
  let imports = [];
  let exports = [];

  try {
    const importSection = sections.find((section) => section.id === 2);
    if (importSection) imports = parseImports(bytes, importSection.start, importSection.end);

    const exportSection = sections.find((section) => section.id === 7);
    if (exportSection) exports = parseExports(bytes, exportSection.start, exportSection.end);
  } catch (error) {
    errors.push(error.message);
  }

  const importFunctionCount = imports.filter((entry) => entry.kind === 'function').length;

  // Function index -> { name, source }. Imports occupy the first slots of the
  // index space, so they are named too and `call 0` reads as `call 0 (log)`.
  // Later writes win: `name` section > export > import.
  const symbols = new Map();
  imports
    .filter((entry) => entry.kind === 'function')
    .forEach((entry, index) => symbols.set(index, { name: entry.name, source: 'export' }));

  for (const entry of exports) {
    if (entry.kind === 'function') symbols.set(entry.index, { name: entry.name, source: 'export' });
  }

  const nameSection = findCustomSection(sections, 'name');
  if (nameSection) {
    try {
      for (const [index, name] of parseNameSectionFunctionNames(
        bytes,
        nameSection.payloadStart,
        nameSection.end,
      )) {
        symbols.set(index, { name, source: 'name' });
      }
    } catch (error) {
      errors.push(`Failed to read "name" section: ${error.message}`);
    }
  }

  const symbolName = (index) => symbols.get(index)?.name;

  const functions = [];
  const flatInstructions = [];
  const codeSection = sections.find((section) => section.id === 10);

  if (!codeSection) {
    errors.push('Module has no code section');
    return {
      ...base,
      valid: true,
      sections: sections.map(summarizeSection),
      imports,
      exports,
      errors,
    };
  }

  let cursor = codeSection.start;
  try {
    const bodyCount = readVarUint32(bytes, cursor);
    cursor = bodyCount.next;
    const total = Math.min(bodyCount.value, maxFunctions);

    if (bodyCount.value > maxFunctions) {
      errors.push(`Code section declares ${bodyCount.value} functions; showing the first ${maxFunctions}`);
    }

    for (let i = 0; i < total && cursor < codeSection.end; i += 1) {
      const size = readVarUint32(bytes, cursor);
      cursor = size.next;
      const bodyStart = cursor;
      const bodyEnd = Math.min(bodyStart + size.value, codeSection.end);
      cursor = bodyEnd;

      const funcIndex = importFunctionCount + i;
      const { locals, instructions } = disassembleFunctionBody(bytes, bodyStart, bodyEnd, {
        localNames: null,
        resolve: (kind, index) => (kind === 'function' ? symbolName(index) : undefined),
      });

      const symbol = symbols.get(funcIndex);
      const funcName = symbol ? symbol.name : `func[${funcIndex}]`;
      const fnRecord = {
        index: funcIndex,
        name: funcName,
        nameSource: symbol ? symbol.source : 'index',
        locals,
        bodyStart,
        bodyEnd,
        truncated: bodyStart + size.value > codeSection.end,
        instructions: instructions.map((instruction, indexInFunction) => ({
          ...instruction,
          funcIndex,
          funcName,
          indexInFunction,
          id: `${funcIndex}:${indexInFunction}`,
        })),
      };

      functions.push(fnRecord);
      for (const instruction of fnRecord.instructions) flatInstructions.push(instruction);
    }
  } catch (error) {
    errors.push(error.message);
  }

  return {
    valid: true,
    byteLength: bytes.length,
    sections: sections.map(summarizeSection),
    imports,
    exports,
    functions,
    instructions: flatInstructions,
    errors,
  };
}

function summarizeSection(section) {
  const summary = { id: section.id, name: section.name, size: section.size };
  if (section.customName) summary.customName = section.customName;
  return summary;
}

/**
 * Case-insensitive opcode filter.
 *
 * Supports a bare mnemonic (`br_if`), a mnemonic plus operand
 * (`call swap`), or a raw hex byte (`0x0f`). Blank input passes everything.
 */
function matchesInstruction(instruction, needle) {
  if (!needle) return true;
  const query = String(needle).trim().toLowerCase();
  if (!query) return true;
  const mnemonic = instruction.mnemonic.toLowerCase();
  if (mnemonic === query) return true;
  if (instruction.text.toLowerCase().includes(query)) return true;
  if (/^0x[0-9a-f]+$/.test(query)) {
    const code = instruction.opcode;
    if (code >= 0 && code.toString(16).padStart(2, '0') === query.slice(2)) return true;
  }
  return false;
}

/** Filter instructions, preserving order and each instruction's global position. */
function filterInstructions(instructions, query) {
  const list = Array.isArray(instructions) ? instructions : [];
  if (!query || !String(query).trim()) return list.slice();
  return list.filter((instruction) => matchesInstruction(instruction, query));
}

/** Distinct mnemonic -> occurrence count, for the filter summary chips. */
function summarizeOpcodes(instructions) {
  const counts = new Map();
  for (const instruction of instructions || []) {
    counts.set(instruction.mnemonic, (counts.get(instruction.mnemonic) || 0) + 1);
  }
  return [...counts.entries()]
    .map(([mnemonic, count]) => ({ mnemonic, count }))
    .sort((a, b) => b.count - a.count || a.mnemonic.localeCompare(b.mnemonic));
}

const RUST_FN_PATTERN =
  /^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:default\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+"[^"]*"\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)/;

/**
 * 1-based line on which the body opened at or after `startIndex` closes, or
 * `null` when the declaration has no body (trait method, extern item). Skips
 * string/char literals and comments so braces inside them don't unbalance the
 * count; raw strings and lifetimes are close enough for a viewer.
 */
function findBodyEndLine(lines, startIndex) {
  let depth = 0;
  let opened = false;
  let inBlockComment = false;

  for (let lineIndex = startIndex; lineIndex < lines.length; lineIndex += 1) {
    const line = lines[lineIndex];
    let inString = false;

    for (let i = 0; i < line.length; i += 1) {
      const ch = line[i];
      const next = line[i + 1];

      if (inBlockComment) {
        if (ch === '*' && next === '/') {
          inBlockComment = false;
          i += 1;
        }
      } else if (inString) {
        if (ch === '\\') i += 1;
        else if (ch === '"') inString = false;
      } else if (ch === '/' && next === '/') {
        break;
      } else if (ch === '/' && next === '*') {
        inBlockComment = true;
        i += 1;
      } else if (ch === '"') {
        inString = true;
      } else if (ch === "'" && line[i + 2] === "'") {
        i += 2; // char literal such as '{'
      } else if (ch === ';' && !opened) {
        return null;
      } else if (ch === '{') {
        depth += 1;
        opened = true;
      } else if (ch === '}') {
        depth -= 1;
        if (opened && depth === 0) return lineIndex + 1;
      }
    }
  }

  return opened ? lines.length : null;
}

/**
 * Locate Rust `fn` definitions and the lines each spans, from the declaration
 * to its closing brace. Bodiless declarations (trait methods) are skipped: they
 * never compile to code, and keeping them would shadow the `impl` that does.
 */
function extractRustFunctionRanges(source) {
  const text = typeof source === 'string' ? source : '';
  if (text.length === 0) return [];

  const lines = text.split(/\r?\n/);
  const ranges = [];

  lines.forEach((line, position) => {
    if (line.trimStart().startsWith('//')) return;
    const match = RUST_FN_PATTERN.exec(line);
    if (!match) return;
    const endLine = findBodyEndLine(lines, position);
    if (endLine === null) return;
    ranges.push({ name: match[1], line: position + 1, endLine });
  });

  return ranges;
}

/**
 * Split a legacy Rust mangled symbol into path segments:
 * `_ZN2lp4swap17h0123456789abcdefE` -> `['lp', 'swap', 'h0123456789abcdef']`.
 * Returns `null` for anything that isn't a well-formed legacy symbol.
 */
function demangleLegacy(symbol) {
  const match = /^_?_ZN(.*)E$/.exec(symbol);
  if (!match) return null;
  const body = match[1];
  const segments = [];
  let cursor = 0;
  while (cursor < body.length) {
    const digits = /^\d+/.exec(body.slice(cursor));
    if (!digits) return null;
    cursor += digits[0].length;
    const size = Number(digits[0]);
    if (size === 0 || cursor + size > body.length) return null;
    segments.push(body.slice(cursor, cursor + size));
    cursor += size;
  }
  return segments.length > 0 ? segments : null;
}

/**
 * Reduce a symbol to a comparable key: demangle legacy `_ZN` symbols, drop the
 * `::h<16 hex>` disambiguator and the crate path, so `swap`,
 * `lp::swap::h0123456789abcdef` and `_ZN2lp4swap17h0123456789abcdefE` all
 * normalise to `swap`.
 */
function normalizeFunctionName(name) {
  if (!name) return '';
  let value = String(name);
  if (value.startsWith('func[')) return value;
  const demangled = demangleLegacy(value);
  if (demangled) value = demangled.join('::');
  value = value.replace(/::h[0-9a-f]{16}$/, '');
  value = value.split('::').pop() || value;
  value = value.replace(/^_+/, '').replace(/_+$/, '');
  return value.toLowerCase();
}

/**
 * Build the Rust-source-line -> opcode-index cross-highlight map.
 *
 * Granularity is function-level, which is what release WASM actually carries:
 * DWARF line tables are stripped from `wasm32-unknown-unknown` artifacts, so
 * anything finer would be fiction. The declaration line owns every opcode of
 * the compiled function; lines inside the body own that function's entry block,
 * i.e. the straight-line prologue before the first branch.
 */
function buildSourceLineMap(source, functions) {
  const ranges = extractRustFunctionRanges(source);
  const byKey = new Map();
  for (const range of ranges) {
    const key = normalizeFunctionName(range.name);
    if (key && !byKey.has(key)) byKey.set(key, range);
  }

  const list = Array.isArray(functions) ? functions : [];
  const lineMap = {};
  const matchedFunctions = [];
  const unmatchedFunctions = [];

  for (const fn of list) {
    const key = normalizeFunctionName(fn.name);
    const range = key ? byKey.get(key) : undefined;

    if (!range) {
      unmatchedFunctions.push(fn.name);
      continue;
    }

    matchedFunctions.push(fn.name);
    const instructionIds = fn.instructions.map((instruction) => instruction.id);
    if (instructionIds.length === 0) continue;

    const entryBlock = entryBlockIds(fn.instructions);

    mergeLineEntries(lineMap, range.line, instructionIds);
    // The closing-brace line is not code, so it owns nothing.
    for (let line = range.line + 1; line < range.endLine; line += 1) {
      mergeLineEntries(lineMap, line, entryBlock);
    }
  }

  return { lineMap, matchedFunctions, unmatchedFunctions };
}

/**
 * Ids of the leading straight-line instruction run, i.e. everything before the
 * first opcode that transfers control (br, br_if, br_table, return, call).
 */
function entryBlockIds(instructions) {
  const ids = [];
  for (const instruction of instructions) {
    const control =
      instruction.mnemonic === 'br' ||
      instruction.mnemonic === 'br_if' ||
      instruction.mnemonic === 'br_table' ||
      instruction.mnemonic === 'return';
    ids.push(instruction.id);
    if (control) break;
  }
  return ids;
}

function mergeLineEntries(lineMap, line, ids) {
  if (ids.length === 0) return;
  if (!lineMap[line]) lineMap[line] = [];
  for (const id of ids) {
    if (!lineMap[line].includes(id)) lineMap[line].push(id);
  }
}

module.exports = {
  OPCODE_TABLE,
  OPCODE_TABLE_FC,
  VALUE_TYPES,
  WasmDisassemblyError,
  toBytes,
  readVarUint32,
  readVarInt32,
  readVarInt64,
  decodeInstruction,
  disassembleFunctionBody,
  disassembleWasm,
  matchesInstruction,
  filterInstructions,
  summarizeOpcodes,
  extractRustFunctionRanges,
  normalizeFunctionName,
  buildSourceLineMap,
};
