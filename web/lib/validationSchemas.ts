import { z } from 'zod';
import type { SorobanType } from './sorobantypes';

const stellarAddressRegex = /^G[A-Z2-7]{55}$/;

const stellarAddressSchema = z
  .string()
  .length(56, 'Stellar address must be exactly 56 characters')
  .regex(stellarAddressRegex, 'Stellar address must start with G followed by 55 valid base32 characters');

const symbolSchema = z
  .string()
  .max(32, 'Symbol must be at most 32 characters')
  .regex(/^[a-zA-Z0-9_]+$/, 'Symbol can only contain letters, numbers, and underscores');

const u32Schema = z
  .string()
  .regex(/^\d+$/, 'u32 must be a non-negative integer')
  .refine((v) => {
    try {
      const n = BigInt(v);
      return n >= 0n && n <= 4294967295n;
    } catch {
      return false;
    }
  }, 'u32 must be between 0 and 4294967295');

const i128Schema = z
  .string()
  .regex(/^-?\d+$/, 'i128 must be an integer')
  .refine((v) => {
    try {
      const n = BigInt(v);
      return n >= -170141183460469231731687303715884105728n && n <= 170141183460469231731687303715884105727n;
    } catch {
      return false;
    }
  }, 'i128 value is out of range');

const u128Schema = z
  .string()
  .regex(/^\d+$/, 'u128 must be a non-negative integer')
  .refine((v) => {
    try {
      const n = BigInt(v);
      return n >= 0n && n <= 340282366920938463463374607431768211455n;
    } catch {
      return false;
    }
  }, 'u128 must be between 0 and 340282366920938463463374607431768211455');

// zod v4 replaced the `errorMap` option with `error`.
const boolSchema = z.enum(['true', 'false'] as const, {
  error: 'Boolean must be true or false',
});

const stringSchema = z.string().max(4096, 'String must be at most 4096 characters');

const passThroughSchema = z.string();

export function getSchema(sorobanType: SorobanType): z.ZodType<string> {
  switch (sorobanType) {
    case 'address':
      return stellarAddressSchema;
    case 'u32':
      return u32Schema;
    case 'i128':
      return i128Schema;
    case 'u128':
      return u128Schema;
    case 'symbol':
      return symbolSchema;
    case 'bool':
      return boolSchema;
    case 'string':
      return stringSchema;
    case 'struct':
    case 'enum':
      return passThroughSchema;
    default:
      return passThroughSchema;
  }
}

export function validateField(sorobanType: SorobanType, value: unknown): { success: boolean; error?: string } {
  const schema = getSchema(sorobanType);
  const result = schema.safeParse(value);
  if (result.success) {
    return { success: true };
  }
  const message = result.error.issues[0]?.message ?? 'Invalid value';
  return { success: false, error: message };
}
