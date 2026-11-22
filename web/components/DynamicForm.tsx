'use client';

import React from "react"

import { useEffect, useState } from 'react';
import type { ContractFunction, SimulationInputs } from '../lib/sorobantypes';
import { Loader2 } from 'lucide-react';
import { validateField } from '../lib/validationSchemas';

import { simulationQueueManager, type RequestQueueStatus } from '../lib/requestQueue';

interface DynamicFormProps {
  func: ContractFunction;
  onSubmit: (inputs: SimulationInputs) => void;
  onInputChange?: (inputs: SimulationInputs) => void;
  liveSimulate?: boolean;
  loading?: boolean;
}

// Soroban spec XDR parameter types that carry structured / complex values.
const COMPLEX_TYPES = ['vector', 'vec', 'struct', 'map', 'tuple', 'option', 'bytes', 'bytesn'];

function isComplexType(type: string): boolean {
  const normalized = type.toLowerCase();
  return COMPLEX_TYPES.some((t) => normalized === t || normalized.startsWith(`${t}<`) || normalized.startsWith(`${t}(`));
}

function isNumericType(type: string): boolean {
  return /^(u|i)(32|64|128|256)$/.test(type.toLowerCase());
}

// Integer types whose range exceeds Number.MAX_SAFE_INTEGER (2^53 - 1).
// These must never be routed through Number()/parseInt()/parseFloat().
function isBigIntType(type: string): boolean {
  return /^(u|i)(64|128|256)$/.test(type.toLowerCase());
}

const INTEGER_PATTERN = /^-?\d+$/;

/**
 * Validates an integer string for a BigInt-backed type without ever converting
 * it to a JS Number. Returns an error message, or null when the value is valid.
 */
export function validateBigIntString(type: string, raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed === '') return null;
  if (!INTEGER_PATTERN.test(trimmed)) {
    return 'Must be an integer (digits only, optional leading -)';
  }
  let parsed: bigint;
  try {
    parsed = BigInt(trimmed);
  } catch {
    return 'Invalid integer value';
  }
  const normalized = type.toLowerCase();
  const unsigned = normalized.startsWith('u');
  if (unsigned && parsed < 0n) {
    return `${type} cannot be negative`;
  }
  const bits = Number(normalized.slice(1));
  if (Number.isFinite(bits) && bits > 0) {
    const min = unsigned ? 0n : -(1n << BigInt(bits - 1));
    const max = unsigned ? (1n << BigInt(bits)) - 1n : (1n << BigInt(bits - 1)) - 1n;
    if (parsed < min || parsed > max) {
      return `Out of range for ${type}`;
    }
  }
  return null;
}

/**
 * Inserts a decimal point into a raw integer digit string without converting to
 * Number, so no precision is lost and scientific notation is never produced.
 * e.g. formatStroops('100000000000000000000', 7) -> '10000000000000.0000000'
 */
export function formatStroops(value: string, decimals = 7): string {
  const trimmed = value.trim();
  if (!INTEGER_PATTERN.test(trimmed)) return value;
  const negative = trimmed.startsWith('-');
  const digits = (negative ? trimmed.slice(1) : trimmed).padStart(decimals + 1, '0');
  const whole = digits.slice(0, digits.length - decimals);
  const fraction = decimals > 0 ? `.${digits.slice(digits.length - decimals)}` : '';
  return `${negative ? '-' : ''}${whole}${fraction}`;
}
=======

export function DynamicForm({ func, onSubmit, onInputChange, liveSimulate = false, loading }: DynamicFormProps) {
  const [formData, setFormData] = useState<SimulationInputs>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [jsonMode, setJsonMode] = useState<Record<string, boolean>>({});
  const [queueStatus, setQueueStatus] = useState<RequestQueueStatus>(() => simulationQueueManager.getStatus());

  useEffect(() => {
    return simulationQueueManager.subscribe(setQueueStatus);
  }, []);

  // A different contract function has different inputs, so values typed for
  // the previous one must not carry over (or be submitted with the new one).
  const selectedFunction = func.name;
  useEffect(() => {
    setFormData({});
    setErrors({});
    setJsonMode({});
  }, [selectedFunction]);

  const handleChange = (name: string, value: string | number | boolean) => {
    // Numeric inputs are kept as raw strings so BigInt-range values (u64/i64/
    // u128/i128) never pass through Number() and lose precision or gain
    // scientific notation.
    const updatedData = { ...formData, [name]: value };
    setFormData(updatedData);
    if (errors[name]) {
      setErrors((prev) => {
        const next = { ...prev };
        delete next[name];
        return next;
      });
    }

    if (onInputChange || liveSimulate) {
      // Throttle contract simulation on change through client-side simulationQueueManager (max 2/sec)
      simulationQueueManager.enqueue(async () => {
        if (onInputChange) {
          onInputChange(updatedData);
        }
      }).catch(() => {});
    }
  };

  const fieldValue = (name: string) => {
    const value = formData[name];
    return typeof value === 'boolean' ? String(value) : value ?? '';
  };

  const toggleJsonMode = (name: string) => {
    setJsonMode((prev) => ({ ...prev, [name]: !prev[name] }));
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    const newErrors: Record<string, string> = {};
    for (const input of func.inputs) {
      const raw = fieldValue(input.name);
      if (!input.optional || raw !== '') {
        if (isComplexType(input.type) && jsonMode[input.name]) {
          try {
            JSON.parse(String(raw));
          } catch {
            newErrors[input.name] = 'Invalid JSON';
            continue;
          }
        }
        if (isBigIntType(input.type)) {
          const bigIntError = validateBigIntString(input.type, String(raw));
          if (bigIntError) {
            newErrors[input.name] = bigIntError;
            continue;
          }
        }
        const result = validateField(input.type, raw);
        if (!result.success) {
          newErrors[input.name] = result.error ?? 'Invalid value';
        }
      }
    }

    if (Object.keys(newErrors).length > 0) {
      setErrors(newErrors);
      return;
    }

    setErrors({});
    onSubmit(formData);
  };

  function inputStyle(hasError: boolean): React.CSSProperties {
    return {
      padding: '8px 12px',
      border: `1px solid ${hasError ? '#f85149' : 'var(--border-default)'}`,
      borderRadius: '6px',
      fontSize: '14px',
      boxSizing: 'border-box',
      backgroundColor: 'var(--bg-input)',
      color: 'var(--text-primary)',
    };
  }

  return (
    <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      {queueStatus.isProcessing && (
        <div
          role="status"
          aria-live="polite"
          style={{
            alignSelf: 'flex-start',
            border: '1px solid rgba(0, 217, 255, 0.35)',
            borderRadius: '999px',
            color: '#00d9ff',
            backgroundColor: 'rgba(0, 217, 255, 0.08)',
            fontSize: '12px',
            padding: '4px 10px',
          }}
        >
          Simulation queue: {queueStatus.waiting} waiting · {queueStatus.active} running · {queueStatus.maxRequestsPerSecond}/sec
        </div>
      )}
      {func.inputs.length === 0 ? (
        <p style={{ color: 'var(--text-secondary)', fontSize: '14px' }}>No inputs required</p>
      ) : (
        func.inputs.map((input) => {
          const hasError = !!errors[input.name];
          const complex = isComplexType(input.type);
          const useJson = complex && jsonMode[input.name];
          const inputId = `contract-input-${encodeURIComponent(input.name)}`;
          const descriptionId = input.description ? `${inputId}-description` : undefined;
          const errorId = hasError ? `${inputId}-error` : undefined;

          return (
          <div
            key={input.name}
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: '4px',
            }}
          >
            <label
              htmlFor={inputId}
              style={{
                fontSize: '14px',
                fontWeight: '500',
                color: 'var(--text-primary)',
              }}
            >
              {input.name}
              {input.optional ? (
                <span style={{ color: 'var(--text-secondary)', marginLeft: '4px' }}>(optional)</span>
              ) : (
                <span style={{ color: '#fb8500' }}>*</span>
              )}
              <span style={{ color: 'var(--text-secondary)', marginLeft: '6px', fontSize: '12px' }}>
                {input.type}
              </span>
            </label>
            {input.description && (
              <p
                id={descriptionId}
                style={{
                  fontSize: '12px',
                  color: 'var(--text-secondary)',
                  margin: '0',
                }}
              >
                {input.description}
              </p>
            )}
            {complex && (
              <button
                type="button"
                onClick={() => toggleJsonMode(input.name)}
                disabled={loading}
                style={{
                  alignSelf: 'flex-start',
                  padding: '2px 8px',
                  fontSize: '12px',
                  backgroundColor: 'transparent',
                  color: '#00d9ff',
                  border: '1px solid var(--border-default)',
                  borderRadius: '4px',
                  cursor: loading ? 'not-allowed' : 'pointer',
                }}
              >
                {useJson ? 'Use structured input' : 'Use JSON input'}
              </button>
            )}
            {useJson ? (
              <textarea
                id={inputId}
                placeholder={`Enter ${input.type} as JSON`}
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
                aria-invalid={hasError || undefined}
                aria-describedby={[descriptionId, errorId].filter(Boolean).join(' ') || undefined}
                rows={4}
                style={{ ...inputStyle(hasError), fontFamily: 'monospace', resize: 'vertical' }}
              />
            ) : input.type === 'address' ? (
              <input
                id={inputId}
                type="text"
                placeholder="Enter Stellar address (G...)"
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
                aria-invalid={hasError || undefined}
                aria-describedby={[descriptionId, errorId].filter(Boolean).join(' ') || undefined}
                style={{ ...inputStyle(hasError), fontFamily: 'monospace' }}
              />
            ) : isNumericType(input.type) ? (
              <>
                <input
                  id={inputId}
                  type="text"
                  inputMode="numeric"
                  placeholder={`Enter ${input.type} value`}
                  value={fieldValue(input.name)}
                  onChange={(e) => handleChange(input.name, e.target.value)}
                  required={!input.optional}
                  disabled={loading}
                  aria-invalid={hasError || undefined}
                  aria-describedby={[descriptionId, errorId].filter(Boolean).join(' ') || undefined}
                  style={inputStyle(hasError)}
                />
                {isBigIntType(input.type) && INTEGER_PATTERN.test(String(fieldValue(input.name)).trim()) && (
                  <p
                    style={{
                      fontSize: '12px',
                      color: 'var(--text-secondary)',
                      margin: '0',
                      fontFamily: 'monospace',
                      wordBreak: 'break-all',
                    }}
                  >
                    {formatStroops(String(fieldValue(input.name)))}
                  </p>
                )}
              </>
            ) : input.type === 'string' || input.type === 'symbol' ? (
              <input
                id={inputId}
                type="text"
                placeholder={`Enter ${input.type}`}
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
                aria-invalid={hasError || undefined}
                aria-describedby={[descriptionId, errorId].filter(Boolean).join(' ') || undefined}
                style={inputStyle(hasError)}
              />
            ) : input.type === 'bool' ? (
              <select
                id={inputId}
                value={formData[input.name] === undefined ? '' : String(formData[input.name])}
                onChange={(e) => handleChange(input.name, e.target.value === 'true')}
                required={!input.optional}
                disabled={loading}
                aria-invalid={hasError || undefined}
                aria-describedby={[descriptionId, errorId].filter(Boolean).join(' ') || undefined}
                style={inputStyle(hasError)}
              >
                <option value="">Select value</option>
                <option value="true">True</option>
                <option value="false">False</option>
              </select>
            ) : (
              <input
                id={inputId}
                type="text"
                placeholder="Enter value"
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
                aria-invalid={hasError || undefined}
                aria-describedby={[descriptionId, errorId].filter(Boolean).join(' ') || undefined}
                style={inputStyle(hasError)}
              />
            )}
            {hasError && (
              <p id={errorId} role="alert" style={{ color: '#f85149', fontSize: '12px', margin: '2px 0 0 0' }}>
                {errors[input.name]}
              </p>
            )}
          </div>
          );
        })
      )}
      <div style={{ display: 'flex', gap: '12px', marginTop: '8px' }}>
        <button
          type="submit"
          disabled={loading}
          style={{
            padding: '10px 20px',
            backgroundColor: loading ? '#30363d' : '#00d9ff',
            color: loading ? '#8b949e' : '#0f1117',
            border: 'none',
            borderRadius: '6px',
            fontSize: '14px',
            fontWeight: '600',
            cursor: loading ? 'not-allowed' : 'pointer',
            flex: 1,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '8px',
          }}
        >
          {loading ? (
            <>
              <Loader2 size={16} className="animate-spin" />
              <span>Simulating...</span>
            </>
          ) : (
            'Simulate'
          )}
        </button>
        <button
          type="button"
          disabled={loading}
          style={{
            padding: '10px 20px',
            backgroundColor: loading ? '#30363d' : '#a371f7',
            color: loading ? '#8b949e' : '#fff',
            border: 'none',
            borderRadius: '6px',
            fontSize: '14px',
            fontWeight: '600',
            cursor: loading ? 'not-allowed' : 'pointer',
            flex: 1,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '8px',
          }}
        >
          {loading ? (
            <>
              <Loader2 size={16} className="animate-spin" />
              <span>Invoking...</span>
            </>
          ) : (
            'Live (Invoke)'
          )}
        </button>
      </div>
    </form>
  );
}
