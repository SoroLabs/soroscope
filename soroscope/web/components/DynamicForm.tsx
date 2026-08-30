'use client';

import React, { useState } from 'react';
import { Loader2 } from 'lucide-react';
import type { ContractFunction, SimulationInputs } from '../lib/sorobantypes';
import { useTransactionToasts } from './ToastViewport';

interface DynamicFormProps {
  func: ContractFunction;
  onSubmit: (inputs: SimulationInputs) => Promise<unknown> | void;
  loading?: boolean;
}

export function DynamicForm({ func, onSubmit, loading }: DynamicFormProps) {
  const [formData, setFormData] = useState<SimulationInputs>({});
  const { showToast } = useTransactionToasts();

  const handleChange = (name: string, value: string | number | boolean) => {
    setFormData((prev) => ({ ...prev, [name]: value }));
  };

  const fieldValue = (name: string) => {
    const value = formData[name];
    return typeof value === 'boolean' ? String(value) : value ?? '';
  };

  const handleSubmit = async (event: React.FormEvent, mode: 'simulate' | 'invoke' = 'simulate') => {
    event.preventDefault();
    if (loading) {
      return;
    }

    if (mode === 'invoke') {
      showToast({ phase: 'signing', message: 'Please review and approve the transaction in your wallet.' });
      await new Promise((resolve) => window.setTimeout(resolve, 250));
      showToast({ phase: 'submitting', message: 'Broadcasting the transaction to the network.' });

      try {
        const result = await Promise.resolve(onSubmit(formData));
        const txHash =
          typeof result === 'object' && result !== null && 'result' in result &&
          typeof (result as { result?: { transaction_hash?: string } }).result?.transaction_hash === 'string'
            ? (result as { result: { transaction_hash: string } }).result.transaction_hash
            : undefined;
        showToast({ phase: 'success', message: 'Transaction completed successfully.', txHash });
      } catch (error) {
        showToast({ phase: 'failed', message: error instanceof Error ? error.message : 'The transaction could not be completed.' });
      }
      return;
    }

    await Promise.resolve(onSubmit(formData));
  };

  return (
    <form onSubmit={(event) => void handleSubmit(event, 'simulate')} style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      {func.inputs.length === 0 ? (
        <p style={{ color: '#8b949e', fontSize: '14px' }}>No inputs required</p>
      ) : (
        func.inputs.map((input) => (
          <div key={input.name} style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
            <label style={{ fontSize: '14px', fontWeight: '500', color: '#c9d1d9' }}>
              {input.name}
              {input.optional ? (
                <span style={{ color: '#8b949e', marginLeft: '4px' }}>(optional)</span>
              ) : (
                <span style={{ color: '#fb8500' }}>*</span>
              )}
            </label>
            {input.description && (
              <p style={{ fontSize: '12px', color: '#8b949e', margin: '0' }}>{input.description}</p>
            )}
            {input.type === 'address' ? (
              <input
                type="text"
                placeholder="Enter Stellar address (G...)"
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
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
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
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
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
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
                value={formData[input.name] === undefined ? '' : String(formData[input.name])}
                onChange={(e) => handleChange(input.name, e.target.value === 'true')}
                required={!input.optional}
                disabled={loading}
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
                value={fieldValue(input.name)}
                onChange={(e) => handleChange(input.name, e.target.value)}
                required={!input.optional}
                disabled={loading}
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
          onClick={(event) => void handleSubmit(event, 'invoke')}
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
