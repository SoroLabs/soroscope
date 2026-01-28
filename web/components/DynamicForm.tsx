'use client';

import React from "react"

import { useState } from 'react';
import type { ContractFunction } from '../lib/sorobantypes';

interface DynamicFormProps {
  func: ContractFunction;
  onSubmit: (inputs: Record<string, any>) => void;
  loading?: boolean;
}

export function DynamicForm({ func, onSubmit, loading }: DynamicFormProps) {
  const [formData, setFormData] = useState<Record<string, any>>({});

  const handleChange = (name: string, value: any) => {
    setFormData((prev) => ({ ...prev, [name]: value }));
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit(formData);
  };

  return (
    <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      {func.inputs.length === 0 ? (
        <p style={{ color: '#8b949e', fontSize: '14px' }}>No inputs required</p>
      ) : (
        func.inputs.map((input) => (
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
                color: '#c9d1d9',
              }}
            >
              {input.name}
              {input.optional ? (
                <span style={{ color: '#8b949e', marginLeft: '4px' }}>(optional)</span>
              ) : (
                <span style={{ color: '#fb8500' }}>*</span>
              )}
            </label>
            {input.description && (
              <p
                style={{
                  fontSize: '12px',
                  color: '#8b949e',
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
                value={formData[input.name] || ''}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                style={{
                  padding: '8px 12px',
                  border: '1px solid #30363d',
                  borderRadius: '6px',
                  fontSize: '14px',
                  fontFamily: 'monospace',
                  boxSizing: 'border-box',
                  backgroundColor: '#0d1117',
                  color: '#c9d1d9',
                }}
              />
            ) : input.type === 'u32' || input.type === 'u128' || input.type === 'i128' ? (
              <input
                type="number"
                placeholder={`Enter ${input.type} value`}
                value={formData[input.name] || ''}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                style={{
                  padding: '8px 12px',
                  border: '1px solid #30363d',
                  borderRadius: '6px',
                  fontSize: '14px',
                  boxSizing: 'border-box',
                  backgroundColor: '#0d1117',
                  color: '#c9d1d9',
                }}
              />
            ) : input.type === 'string' || input.type === 'symbol' ? (
              <input
                type="text"
                placeholder={`Enter ${input.type}`}
                value={formData[input.name] || ''}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                style={{
                  padding: '8px 12px',
                  border: '1px solid #30363d',
                  borderRadius: '6px',
                  fontSize: '14px',
                  boxSizing: 'border-box',
                  backgroundColor: '#0d1117',
                  color: '#c9d1d9',
                }}
              />
            ) : input.type === 'bool' ? (
              <select
                value={formData[input.name] === undefined ? '' : formData[input.name]}
                onChange={(e) => handleChange(input.name, e.target.value === 'true')}
                required={!input.optional}
                style={{
                  padding: '8px 12px',
                  border: '1px solid #30363d',
                  borderRadius: '6px',
                  fontSize: '14px',
                  boxSizing: 'border-box',
                  backgroundColor: '#0d1117',
                  color: '#c9d1d9',
                }}
              >
                <option value="">Select value</option>
                <option value="true">True</option>
                <option value="false">False</option>
              </select>
            ) : (
              <input
                type="text"
                placeholder="Enter value"
                value={formData[input.name] || ''}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                style={{
                  padding: '8px 12px',
                  border: '1px solid #30363d',
                  borderRadius: '6px',
                  fontSize: '14px',
                  boxSizing: 'border-box',
                  backgroundColor: '#0d1117',
                  color: '#c9d1d9',
                }}
              />
            )}
          </div>
        ))
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
          }}
        >
          {loading ? 'Simulating...' : 'Simulate'}
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
          }}
        >
          {loading ? 'Invoking...' : 'Live (Invoke)'}
        </button>
      </div>
    </form>
  );
}
