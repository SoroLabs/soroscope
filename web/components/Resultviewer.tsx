'use client';

import type { InvocationResult } from '../lib/sorobantypes';

interface ResultViewerProps {
  result: InvocationResult | null;
}

export function ResultViewer({ result }: ResultViewerProps) {
  if (!result) {
    return (
      <div
        style={{
          padding: '24px',
          backgroundColor: '#0d1117',
          borderRadius: '8px',
          textAlign: 'center',
          color: '#8b949e',
          border: '1px solid #30363d',
        }}
      >
        <p>No results yet. Execute a contract function to see results here.</p>
      </div>
    );
  }

  return (
    <div
      style={{
        padding: '24px',
        backgroundColor: '#0d1117',
        borderRadius: '8px',
        borderLeft: `4px solid ${result.success ? '#00d9ff' : '#fb8500'}`,
        border: `1px solid #30363d`,
      }}
    >
      <div style={{ marginBottom: '16px' }}>
        <h3
          style={{
            margin: '0 0 8px 0',
            color: result.success ? '#00d9ff' : '#fb8500',
            fontSize: '16px',
            fontWeight: '600',
          }}
        >
          {result.success ? '✓ Success' : '✗ Error'}
        </h3>
        <p style={{ margin: '0', color: '#8b949e', fontSize: '13px' }}>
          {new Date(result.timestamp).toLocaleString()}
        </p>
      </div>

      {result.error ? (
        <div
          style={{
            backgroundColor: '#0d1117',
            padding: '12px',
            borderRadius: '6px',
            marginBottom: '12px',
            fontSize: '13px',
            color: '#fb8500',
            fontFamily: 'monospace',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-all',
            border: '1px solid #30363d',
          }}
        >
          {result.error}
        </div>
      ) : (
        result.result && (
          <div
            style={{
              backgroundColor: '#0d1117',
              padding: '12px',
              borderRadius: '6px',
              marginBottom: '12px',
              fontSize: '13px',
              fontFamily: 'monospace',
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-all',
              color: '#58a6ff',
              border: '1px solid #30363d',
            }}
          >
            <strong>Result:</strong>
            <br />
            {JSON.stringify(result.result, null, 2)}
          </div>
        )
      )}

      {result.resourceCost && (
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: '1fr 1fr',
            gap: '12px',
            fontSize: '13px',
          }}
        >
          <div
            style={{
              backgroundColor: '#0d1117',
              padding: '12px',
              borderRadius: '6px',
              border: '1px solid #30363d',
            }}
          >
            <strong style={{ color: '#00d9ff' }}>Fee:</strong>
            <p style={{ margin: '4px 0 0 0', color: '#8b949e' }}>{result.resourceCost.fee} XLM</p>
          </div>
          <div
            style={{
              backgroundColor: '#0d1117',
              padding: '12px',
              borderRadius: '6px',
              border: '1px solid #30363d',
            }}
          >
            <strong style={{ color: '#00d9ff' }}>Instructions:</strong>
            <p style={{ margin: '4px 0 0 0', color: '#8b949e' }}>
              {result.resourceCost.instructions}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
