// validationSchemas.test.cjs — unit tests for Soroban type validation schemas
// Runs with: node --test ./lib/validationSchemas.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// Inline validation logic (mirrors validationSchemas.ts for isolated testing without TS transpilation)

function validateField(sorobanType, value) {
  switch (sorobanType) {
    case 'address':
      if (typeof value !== 'string' || value.length !== 56) return { success: false, error: 'Stellar address must be exactly 56 characters' };
      if (!/^G[A-Z2-7]{55}$/.test(value)) return { success: false, error: 'Stellar address must start with G followed by 55 valid base32 characters' };
      return { success: true };
    case 'u32':
      if (!/^\d+$/.test(value)) return { success: false, error: 'u32 must be a non-negative integer' };
      try { const n = BigInt(value); if (n < 0n || n > 4294967295n) return { success: false, error: 'u32 must be between 0 and 4294967295' }; } catch { return { success: false, error: 'u32 value is out of range' }; }
      return { success: true };
    case 'i128':
      if (!/^-?\d+$/.test(value)) return { success: false, error: 'i128 must be an integer' };
      try { const n = BigInt(value); if (n < -170141183460469231731687303715884105728n || n > 170141183460469231731687303715884105727n) return { success: false, error: 'i128 value is out of range' }; } catch { return { success: false, error: 'i128 value is out of range' }; }
      return { success: true };
    case 'u128':
      if (!/^\d+$/.test(value)) return { success: false, error: 'u128 must be a non-negative integer' };
      try { const n = BigInt(value); if (n < 0n || n > 340282366920938463463374607431768211455n) return { success: false, error: 'u128 must be between 0 and 340282366920938463463374607431768211455' }; } catch { return { success: false, error: 'u128 value is out of range' }; }
      return { success: true };
    case 'symbol':
      if (typeof value !== 'string' || value.length > 32) return { success: false, error: 'Symbol must be at most 32 characters' };
      if (!/^[a-zA-Z0-9_]+$/.test(value)) return { success: false, error: 'Symbol can only contain letters, numbers, and underscores' };
      return { success: true };
    case 'bool':
      if (value !== 'true' && value !== 'false') return { success: false, error: 'Boolean must be true or false' };
      return { success: true };
    case 'string':
      if (typeof value !== 'string') return { success: false, error: 'String value required' };
      if (value.length > 4096) return { success: false, error: 'String must be at most 4096 characters' };
      return { success: true };
    case 'struct':
    case 'enum':
      return { success: true };
  }
}

// ── Address Tests ──

test('validateField: valid Stellar address passes', () => {
  const result = validateField('address', 'GAX23V3WWDPPR5WRER3KTEUTDLSCGZYMSJY5FDRRKKCIQ4JADF5T27RC');
  assert.equal(result.success, true);
});

test('validateField: address with wrong prefix (not G) fails', () => {
  const result = validateField('address', 'BAX23V3WWDPPR5WRER3KTEUTDLSCGZYMSJY5FDRRKKCIQ4JADF5T27RC');
  assert.equal(result.success, false);
  assert.ok(result.error);
});

test('validateField: address with wrong length fails', () => {
  const result = validateField('address', 'SHORT');
  assert.equal(result.success, false);
  assert.ok(result.error);
});

test('validateField: address with invalid characters fails', () => {
  const result = validateField('address', 'G0I!23V3WWDPPR5WRER3KTEUTDLSCGZYMSJY5FDRRKKCIQ4JADF5T27RC');
  assert.equal(result.success, false);
  assert.ok(result.error);
});

// ── u32 Tests ──

test('validateField: valid u32 passes', () => {
  assert.equal(validateField('u32', '0').success, true);
  assert.equal(validateField('u32', '100').success, true);
  assert.equal(validateField('u32', '4294967295').success, true);
});

test('validateField: u32 negative number fails', () => {
  const result = validateField('u32', '-1');
  assert.equal(result.success, false);
});

test('validateField: u32 overflow fails', () => {
  const result = validateField('u32', '4294967296');
  assert.equal(result.success, false);
});

test('validateField: u32 non-numeric string fails', () => {
  const result = validateField('u32', 'abc');
  assert.equal(result.success, false);
});

// ── i128 Tests ──

test('validateField: valid i128 passes', () => {
  assert.equal(validateField('i128', '0').success, true);
  assert.equal(validateField('i128', '-100').success, true);
  assert.equal(validateField('i128', '170141183460469231731687303715884105727').success, true);
  assert.equal(validateField('i128', '-170141183460469231731687303715884105728').success, true);
});

test('validateField: i128 overflow fails', () => {
  const result = validateField('i128', '170141183460469231731687303715884105728');
  assert.equal(result.success, false);
});

// ── u128 Tests ──

test('validateField: valid u128 passes', () => {
  assert.equal(validateField('u128', '0').success, true);
  assert.equal(validateField('u128', '340282366920938463463374607431768211455').success, true);
});

test('validateField: u128 negative fails', () => {
  const result = validateField('u128', '-1');
  assert.equal(result.success, false);
});

// ── Symbol Tests ──

test('validateField: valid symbol passes', () => {
  assert.equal(validateField('symbol', 'transfer').success, true);
  assert.equal(validateField('symbol', 'ABC_123').success, true);
});

test('validateField: symbol too long fails', () => {
  const result = validateField('symbol', 'a'.repeat(33));
  assert.equal(result.success, false);
});

test('validateField: symbol with special characters fails', () => {
  const result = validateField('symbol', 'hello-world');
  assert.equal(result.success, false);
});

// ── Bool Tests ──

test('validateField: valid bool passes', () => {
  assert.equal(validateField('bool', 'true').success, true);
  assert.equal(validateField('bool', 'false').success, true);
});

test('validateField: invalid bool fails', () => {
  const result = validateField('bool', 'maybe');
  assert.equal(result.success, false);
});

// ── String Tests ──

test('validateField: valid string passes', () => {
  assert.equal(validateField('string', 'hello').success, true);
});

test('validateField: string too long fails', () => {
  const result = validateField('string', 'x'.repeat(4097));
  assert.equal(result.success, false);
});

// ── Pass-through Tests ──

test('validateField: struct passes through', () => {
  assert.equal(validateField('struct', 'anything').success, true);
});

test('validateField: enum passes through', () => {
  assert.equal(validateField('enum', 'anything').success, true);
});
