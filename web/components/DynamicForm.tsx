'use client';

import React from "react"

import { useState } from 'react';
import type { ContractFunction, SimulationInputs } from '../lib/sorobantypes';
import { Loader2 } from 'lucide-react';
import { validateField } from '../lib/validationSchemas';

import { simulationQueueManager } from '../lib/requestQueue';

interface DynamicFormProps {
  func: ContractFunction;
  onSubmit: (inputs: SimulationInputs) => void;
  onInputChange?: (inputs: SimulationInputs) => void;
  liveSimulate?: boolean;
  loading?: boolean;
}

export function DynamicForm({ func, onSubmit, onInputChange, liveSimulate = false, loading }: DynamicFormProps) {
  const [formData, setFormData] = useState<SimulationInputs>({});
  const [errors, setErrors] = useState<Record<string, string>>({});

  const handleChange = (name: string, value: string | number | boolean) => {
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

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    const newErrors: Record<string, string> = {};
    for (const input of func.inputs) {
      const raw = fieldValue(input.name);
      if (!input.optional || raw !== '') {
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
      {func.inputs.length === 0 ? (
        <p style={{ color: 'var(--text-secondary)', fontSize: '14px' }}>No inputs required</p>
      ) : (
        func.inputs.map((input) => {
          const hasError = !!errors[input.name];

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
            </label>
            {input.description && (
              <p
                style={{
                  fontSize: '12px',
                  color: 'var(--text-secondary)',
                  margin: '0',
                }}
              >
                {input.description}
              </p>
            )}
            {input.type === 'address' ? (
              <input
                type="text"
                placeholder="Enter Stellar address (G...)"
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
                style={{ ...inputStyle(hasError), fontFamily: 'monospace' }}
              />
            ) : input.type === 'u32' || input.type === 'u128' || input.type === 'i128' ? (
              <input
                type="text"
                placeholder={`Enter ${input.type} value`}
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
                style={inputStyle(hasError)}
              />
            ) : input.type === 'string' || input.type === 'symbol' ? (
              <input
                type="text"
                placeholder={`Enter ${input.type}`}
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
                style={inputStyle(hasError)}
              />
            ) : input.type === 'bool' ? (
              <select
                value={formData[input.name] === undefined ? '' : String(formData[input.name])}
                onChange={(e) => handleChange(input.name, e.target.value === 'true')}
                required={!input.optional}
                disabled={loading}
                style={inputStyle(hasError)}
              >
                <option value="">Select value</option>
                <option value="true">True</option>
                <option value="false">False</option>
              </select>
            ) : (
              <input
                type="text"
                placeholder="Enter value"
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
                style={inputStyle(hasError)}
              />
            )}
            {hasError && (
              <p style={{ color: '#f85149', fontSize: '12px', margin: '2px 0 0 0' }}>
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
