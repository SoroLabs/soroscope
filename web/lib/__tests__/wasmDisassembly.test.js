const assert = require('node:assert/strict');
const { test, describe } = require('node:test');
const {
  OPCODE_TABLE,
  readVarUint32,
  readVarInt32,
  decodeInstruction,
  disassembleWasm,
  matchesInstruction,
  filterInstructions,
  summarizeOpcodes,
  extractRustFunctionRanges,
  normalizeFunctionName,
  buildSourceLineMap,
} = require('../wasmDisassembly');

/** Encode an unsigned LEB128 integer. */
function uleb(value) {
  const out = [];
  let remaining = value;
  do {
    let byte = remaining & 0x7f;
    remaining >>>= 7;
    if (remaining !== 0) byte |= 0x80;
    out.push(byte);
  } while (remaining !== 0);
  return out;
}

/** Encode a signed LEB128 integer (single-byte fast path plus generic loop). */
function sleb(value) {
  const out = [];
  let more = true;
  let current = value;
  while (more) {
    let byte = current & 0x7f;
    current >>= 7;
    if ((current === 0 && (byte & 0x40) === 0) || (current === -1 && (byte & 0x40) !== 0)) {
      more = false;
    } else {
      byte |= 0x80;
    }
    out.push(byte);
  }
  return out;
}

function name(text) {
  const encoded = Array.from(Buffer.from(text, 'utf8'));
  return [...uleb(encoded.length), ...encoded];
}

function section(id, payload) {
  return [id, ...uleb(payload.length), ...payload];
}

function vec(entries) {
  return [...uleb(entries.length), ...entries.flat()];
}

const MAGIC = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

function module(...sections) {
  return Uint8Array.from([...MAGIC, ...sections.flat()]);
}

/** `() -> ()` type section with a single type entry. */
function emptyTypeSection() {
  return section(1, vec([[0x60, ...vec([]), ...vec([])]]));
}

function functionSection(count) {
  return section(3, vec(Array.from({ length: count }, () => uleb(0))));
}

function codeSection(bodies) {
  return section(
    10,
    vec(bodies.map((body) => [...uleb(body.length), ...body])),
  );
}

/** A body with no locals followed by the given instruction bytes. */
function body(instructions) {
  return [...uleb(0), ...instructions];
}

function exportFunction(nameText, index) {
  return [...name(nameText), 0x00, ...uleb(index)];
}

describe('WASM LEB128 primitives', () => {
  test('readVarUint32 decodes single and multi byte integers', () => {
    const bytes = Uint8Array.from([0x7f, 0xe5, 0x8e, 0x26]);
    assert.deepEqual(readVarUint32(bytes, 0), { value: 127, next: 1 });
    assert.deepEqual(readVarUint32(bytes, 1), { value: 624485, next: 4 });
  });

  test('readVarInt32 decodes signed values including sign extension', () => {
    assert.deepEqual(readVarInt32(Uint8Array.from(sleb(-1)), 0), { value: -1, next: 1 });
    assert.deepEqual(readVarInt32(Uint8Array.from(sleb(63)), 0), { value: 63, next: 1 });
    assert.deepEqual(readVarInt32(Uint8Array.from(sleb(-64)), 0), { value: -64, next: 1 });
    assert.deepEqual(readVarInt32(Uint8Array.from(sleb(624485)), 0), { value: 624485, next: 3 });
  });
});

describe('decodeInstruction', () => {
  const ctx = { resolve: (kind, index) => (kind === 'function' && index === 3 ? 'swap' : undefined) };

  test('decodes opcodes without immediates', () => {
    const { instruction, next } = decodeInstruction(Uint8Array.from([0x01]), 0);
    assert.equal(instruction.mnemonic, 'nop');
    assert.equal(instruction.text, 'nop');
    assert.equal(next, 1);
    assert.equal(instruction.unknown, false);
  });

  test('decodes i32.const immediates', () => {
    const { instruction } = decodeInstruction(Uint8Array.from([0x41, ...sleb(-12345)]), 0);
    assert.equal(instruction.mnemonic, 'i32.const');
    assert.deepEqual(instruction.operands, ['-12345']);
    assert.equal(instruction.text, 'i32.const -12345');
  });

  test('decodes call operands and resolves the target name', () => {
    const bytes = Uint8Array.from([0x10, ...uleb(3)]);
    const { instruction } = decodeInstruction(bytes, 0, ctx);
    assert.equal(instruction.mnemonic, 'call');
    assert.deepEqual(instruction.operands, ['3 (swap)']);
  });

  test('decodes br_table targets including the default label', () => {
    const bytes = Uint8Array.from([0x0e, ...uleb(2), ...uleb(0), ...uleb(1), ...uleb(3)]);
    const { instruction, next } = decodeInstruction(bytes, 0);
    assert.equal(instruction.mnemonic, 'br_table');
    assert.deepEqual(instruction.operands, ['0 1 3']);
    assert.equal(next, bytes.length);
  });

  test('decodes memarg alignment and offset', () => {
    const bytes = Uint8Array.from([0x28, ...uleb(2), ...uleb(64)]);
    const { instruction } = decodeInstruction(bytes, 0);
    assert.equal(instruction.mnemonic, 'i32.load');
    assert.deepEqual(instruction.operands, ['offset=64 align=2']);
  });

  test('decodes the void block type', () => {
    const { instruction } = decodeInstruction(Uint8Array.from([0x02, 0x40]), 0);
    assert.equal(instruction.mnemonic, 'block');
    assert.deepEqual(instruction.operands, ['void']);
  });

  test('decodes 0xfc prefixed opcodes', () => {
    const { instruction } = decodeInstruction(Uint8Array.from([0xfc, 0x00]), 0);
    assert.equal(instruction.mnemonic, 'i32.trunc_sat_f32_s');
    assert.equal(instruction.size, 2);
  });

  test('flags unknown opcodes instead of throwing', () => {
    const { instruction } = decodeInstruction(Uint8Array.from([0xd5]), 0);
    assert.equal(instruction.unknown, true);
    assert.equal(instruction.mnemonic, 'unknown.0xd5');
  });

  test('exposes a non-empty opcode table with a matching immediate kind', () => {
    assert.ok(Object.keys(OPCODE_TABLE).length > 100);
    for (const entry of Object.values(OPCODE_TABLE)) {
      assert.match(entry, /^[a-z0-9_.]+:[a-z0-9]+$/);
    }
  });
});

describe('disassembleWasm', () => {
  test('rejects input without the WASM magic number', () => {
    const result = disassembleWasm(Uint8Array.from([1, 2, 3]));
    assert.equal(result.valid, false);
    assert.match(result.errors[0], /too small/i);
  });

  test('surfaces a truncated-section error instead of throwing', () => {
    const result = disassembleWasm(Uint8Array.from([...MAGIC, 0x01, 0x7f]));
    assert.equal(result.valid, false);
    assert.equal(result.errors.length, 1);
  });

  test('decodes a module and names functions from the export section', () => {
    const bytes = module(
      emptyTypeSection(),
      functionSection(1),
      section(7, vec([exportFunction('swap', 0)])),
      codeSection([body([0x41, ...sleb(42), 0x1a, 0x0b])]),
    );

    const result = disassembleWasm(bytes);
    assert.equal(result.valid, true);
    assert.deepEqual(result.errors, []);
    assert.equal(result.functions.length, 1);

    const fn = result.functions[0];
    assert.equal(fn.name, 'swap');
    assert.equal(fn.nameSource, 'export');
    assert.equal(fn.index, 0);
    assert.deepEqual(
      fn.instructions.map((instruction) => instruction.mnemonic),
      ['i32.const', 'drop', 'end'],
    );
    assert.equal(fn.instructions[0].operands[0], '42');
    assert.deepEqual(fn.instructions.map((instruction) => instruction.id), ['0:0', '0:1', '0:2']);
    assert.equal(result.instructions.length, 3);
  });

  test('prefers the name custom section over export names', () => {
    const nameSection = section(
      0,
      [
        ...name('name'),
        0x01,
        ...uleb(1 + uleb(0).length + name('liquidity_pool::swap').length),
        ...uleb(1), // one naming entry
        ...uleb(0),
        ...name('liquidity_pool::swap'),
      ],
    );

    const bytes = module(
      emptyTypeSection(),
      functionSection(1),
      section(7, vec([exportFunction('swap', 0)])),
      codeSection([body([0x01, 0x0b])]),
      nameSection,
    );

    const result = disassembleWasm(bytes);
    assert.equal(result.functions[0].name, 'liquidity_pool::swap');
    assert.equal(result.functions[0].nameSource, 'name');
  });

  test('offsets the function index space by imported functions', () => {
    const importSection = section(2, vec([[...name('env'), ...name('log'), 0x00, ...uleb(0)]]));
    const nameSection = section(
      0,
      [
        ...name('name'),
        0x01,
        ...uleb(1 + uleb(1).length + name('lp::deposit').length),
        ...uleb(1), // one naming entry
        ...uleb(1),
        ...name('lp::deposit'),
      ],
    );

    const bytes = module(
      importSection,
      emptyTypeSection(),
      functionSection(1),
      codeSection([body([0x10, ...uleb(0), 0x0b])]),
      nameSection,
    );

    const result = disassembleWasm(bytes);
    assert.equal(result.imports.length, 1);
    assert.equal(result.imports[0].module, 'env');
    assert.equal(result.imports[0].kind, 'function');
    assert.equal(result.functions[0].index, 1);
    assert.equal(result.functions[0].name, 'lp::deposit');
    assert.equal(result.functions[0].instructions[0].mnemonic, 'call');
    assert.deepEqual(result.functions[0].instructions[0].operands, ['0 (log)']);
  });

  test('decodes locals, branches and multiple functions', () => {
    const branchyBody = [
      ...uleb(1),
      ...uleb(2),
      0x7f,
      ...uleb(0),
      0x0d,
      ...uleb(0),
      0x0b,
      0x0b,
    ];

    const bytes = module(
      emptyTypeSection(),
      functionSection(2),
      codeSection([body([0x01, 0x0b]), branchyBody]),
    );

    const result = disassembleWasm(bytes);
    assert.equal(result.functions[0].name, 'func[0]');
    assert.equal(result.functions[0].nameSource, 'index');
    assert.deepEqual(result.functions[1].locals, ['i32', 'i32']);
    assert.equal(result.functions[1].instructions[1].mnemonic, 'br_if');
    assert.equal(result.functions[1].instructions[1].operands[0], '0');
  });

  test('reports a missing code section without failing the whole decode', () => {
    const result = disassembleWasm(module(emptyTypeSection(), functionSection(1)));
    assert.equal(result.valid, true);
    assert.deepEqual(result.functions, []);
    assert.match(result.errors[0], /no code section/i);
  });

  test('caps the number of decoded functions', () => {
    const bytes = module(
      emptyTypeSection(),
      functionSection(4),
      codeSection(Array.from({ length: 4 }, () => body([0x01, 0x0b]))),
    );

    const result = disassembleWasm(bytes, { maxFunctions: 2 });
    assert.equal(result.functions.length, 2);
    assert.match(result.errors[0], /showing the first 2/);
  });
});

describe('opcode filtering', () => {
  const instructions = [
    { mnemonic: 'i32.const', text: 'i32.const 42', opcode: 0x41 },
    { mnemonic: 'br_if', text: 'br_if 0', opcode: 0x0d },
    { mnemonic: 'call', text: 'call 3 (swap)', opcode: 0x10 },
  ];

  test('matches mnemonics case-insensitively', () => {
    assert.equal(matchesInstruction(instructions[1], 'BR_IF'), true);
    assert.equal(matchesInstruction(instructions[1], 'Br'), true, 'a prefix finds the whole br family');
    assert.equal(matchesInstruction(instructions[1], 'loop'), false);
  });

  test('matches substrings across operands', () => {
    assert.equal(matchesInstruction(instructions[2], 'swap'), true);
    assert.equal(matchesInstruction(instructions[2], 'deposit'), false);
  });

  test('matches raw hex opcodes', () => {
    assert.equal(matchesInstruction(instructions[0], '0x41'), true);
    assert.equal(matchesInstruction(instructions[0], '0x10'), false);
  });

  test('an empty query matches everything', () => {
    assert.equal(filterInstructions(instructions, '').length, 3);
    assert.equal(filterInstructions(instructions, '   ').length, 3);
  });

  test('returns only matching instructions and preserves order', () => {
    const result = filterInstructions(instructions, 'b');
    assert.deepEqual(result.map((instruction) => instruction.mnemonic), ['br_if']);
  });

  test('tolerates a non-array input', () => {
    assert.deepEqual(filterInstructions(null, 'br'), []);
  });

  test('summarises mnemonic frequency, most common first', () => {
    const summary = summarizeOpcodes([
      { mnemonic: 'drop' },
      { mnemonic: 'i32.add' },
      { mnemonic: 'drop' },
    ]);
    assert.deepEqual(summary, [
      { mnemonic: 'drop', count: 2 },
      { mnemonic: 'i32.add', count: 1 },
    ]);
  });
});

describe('Rust source line mapping', () => {
  const source = [
    'use soroban_sdk::contract;',                                  // 1
    '',                                                            // 2
    'pub fn swap(env: Env, amount: u128) -> u128 {',               // 3
    '    let fee = amount / 100;',                                 // 4
    '    env.events().publish((env, symbol_short!("swap")))',       // 5
    '}',                                                           // 6
    '',                                                            // 7
    'fn helper(x: u32) -> u32 {',                                  // 8
    '    x + 1',                                                   // 9
    '}',                                                           // 10
  ].join('\n');

  test('extracts declarations with their owned line ranges', () => {
    const ranges = extractRustFunctionRanges(source);
    assert.deepEqual(ranges, [
      { name: 'swap', line: 3, endLine: 6 },
      { name: 'helper', line: 8, endLine: 10 },
    ]);
  });

  test('skips bodiless trait declarations and braces inside strings', () => {
    const ranges = extractRustFunctionRanges(
      [
        'trait Pool {',                   // 1
        '    fn swap(e: Env) -> u32;',    // 2
        '}',                              // 3
        'impl Pool for P {',              // 4
        '    fn swap(e: Env) -> u32 {',   // 5
        '        let s = "}";',           // 6
        '        1',                      // 7
        '    }',                          // 8
        '}',                              // 9
      ].join('\n'),
    );
    assert.deepEqual(ranges, [{ name: 'swap', line: 5, endLine: 8 }]);
  });

  test('ignores commented-out declarations', () => {
    const ranges = extractRustFunctionRanges('// pub fn ghost() {}\npub fn real() {}');
    assert.equal(ranges.length, 1);
    assert.equal(ranges[0].name, 'real');
  });

  test('handles modifiers, generics and empty input', () => {
    const ranges = extractRustFunctionRanges(
      'pub(crate) async unsafe fn go<T>(a: T) -> T {}\nimpl Foo {\n    pub const fn x() {}\n}',
    );
    assert.deepEqual(ranges.map((range) => range.name), ['go', 'x']);
    assert.deepEqual(extractRustFunctionRanges(''), []);
    assert.deepEqual(extractRustFunctionRanges(undefined), []);
  });

  test('normalises symbols by stripping crate paths and legacy suffixes', () => {
    assert.equal(normalizeFunctionName('liquidity_pool::swap'), 'swap');
    assert.equal(normalizeFunctionName('lp::swap::h0123456789abcdef'), 'swap');
    assert.equal(normalizeFunctionName('_ZN2lp4swap17h0123456789abcdefE'), 'swap');
    assert.equal(normalizeFunctionName('_ZNnot_mangled'), 'znnot_mangled');
    assert.equal(normalizeFunctionName('func[7]'), 'func[7]');
    assert.equal(normalizeFunctionName(''), '');
  });

  test('maps a declaration line to every opcode of the compiled function', () => {
    const functions = [
      {
        name: 'lp::swap',
        instructions: [
          { id: '1:0', mnemonic: 'i32.const' },
          { id: '1:1', mnemonic: 'br_if' },
          { id: '1:2', mnemonic: 'end' },
        ],
      },
    ];

    const { lineMap, matchedFunctions } = buildSourceLineMap(source, functions);
    assert.deepEqual(matchedFunctions, ['lp::swap']);
    assert.deepEqual(lineMap[3], ['1:0', '1:1', '1:2']);
  });

  test('maps body lines to the function entry block only', () => {
    const functions = [
      {
        name: 'swap',
        instructions: [
          { id: '0:0', mnemonic: 'local.get' },
          { id: '0:1', mnemonic: 'i32.const' },
          { id: '0:2', mnemonic: 'br_if' },
          { id: '0:3', mnemonic: 'i64.const' },
        ],
      },
    ];

    const { lineMap } = buildSourceLineMap(source, functions);
    assert.deepEqual(lineMap[4], ['0:0', '0:1', '0:2']);
    assert.deepEqual(lineMap[5], ['0:0', '0:1', '0:2']);
    assert.equal(lineMap[6], undefined, 'closing brace is outside the mapped range');
  });

  test('leaves unrelated source lines unmapped', () => {
    const functions = [
      { name: 'swap', instructions: [{ id: '0:0', mnemonic: 'nop' }] },
    ];
    const { lineMap } = buildSourceLineMap(source, functions);
    assert.equal(lineMap[1], undefined);
    assert.equal(lineMap[8], undefined);
  });

  test('reports functions that have no Rust counterpart', () => {
    const functions = [
      { name: 'soroban_sdk::host::call', instructions: [{ id: '0:0', mnemonic: 'nop' }] },
    ];
    const { lineMap, matchedFunctions, unmatchedFunctions } = buildSourceLineMap(source, functions);
    assert.deepEqual(lineMap, {});
    assert.deepEqual(matchedFunctions, []);
    assert.deepEqual(unmatchedFunctions, ['soroban_sdk::host::call']);
  });

  test('tolerates missing source and function lists', () => {
    assert.deepEqual(buildSourceLineMap('', []), { lineMap: {}, matchedFunctions: [], unmatchedFunctions: [] });
    assert.deepEqual(buildSourceLineMap(source, undefined).lineMap, {});
  });

  test('merges multiple functions that share a normalised name', () => {
    const functions = [
      { name: 'a::go', instructions: [{ id: '0:0', mnemonic: 'nop' }] },
      { name: 'b::go', instructions: [{ id: '1:0', mnemonic: 'nop' }] },
    ];
    const { lineMap, matchedFunctions } = buildSourceLineMap('fn go() {}', functions);
    assert.deepEqual(matchedFunctions, ['a::go', 'b::go']);
    assert.deepEqual(lineMap[1], ['0:0', '1:0']);
  });
});
