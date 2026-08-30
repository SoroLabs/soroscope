'use client';

import type { InvocationResult } from '../lib/sorobantypes';

import { CallGraphVisualizer } from './CallGraphVisualizer';

interface ResultViewerProps {
  result: InvocationResult | null;
}

export function ResultViewer({ result }: ResultViewerProps) {
  const downloadSnapshot = () => {
    if (!result?.stateSnapshot) return;
    const blob = new Blob([JSON.stringify(result.stateSnapshot, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `soroscope-snapshot-${result.functionName}-${Date.now()}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  const exportReport = () => {
    if (!result) return;
    const blob = new Blob([JSON.stringify(result, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `soroscope-report-${result.functionName}-${Date.now()}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

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
      <div style={{ marginBottom: '16px', display: 'flex', flexWrap: 'wrap', gap: '12px', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h3
            style={{
              margin: '0 0 4px 0',
              color: result.success ? '#00d9ff' : '#fb8500',
              fontSize: '16px',
              fontWeight: '600',
            }}
          >
            {result.success ? '✓ Success' : '✗ Error'}
          </h3>
          <p style={{ margin: '0', color: '#8b949e', fontSize: '12px' }}>
            {new Date(result.timestamp).toLocaleString()}
          </p>
        </div>
        
        <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
          <button
            onClick={exportReport}
            style={{
              padding: '6px 12px',
              backgroundColor: '#1f2937',
              color: '#f3f4f6',
              borderRadius: '6px',
              border: '1px solid #374151',
              fontSize: '12px',
              cursor: 'pointer',
              transition: 'background-color 0.2s',
            }}
            onMouseOver={(e) => (e.currentTarget.style.backgroundColor = '#374151')}
            onMouseOut={(e) => (e.currentTarget.style.backgroundColor = '#1f2937')}
          >
            Export JSON
          </button>
          {result.stateSnapshot && (
            <button
              onClick={downloadSnapshot}
              style={{
                padding: '6px 12px',
                backgroundColor: '#1f2937',
                color: '#f3f4f6',
                borderRadius: '6px',
                border: '1px solid #374151',
                fontSize: '12px',
                cursor: 'pointer',
                transition: 'background-color 0.2s',
              }}
              onMouseOver={(e) => (e.currentTarget.style.backgroundColor = '#374151')}
              onMouseOut={(e) => (e.currentTarget.style.backgroundColor = '#1f2937')}
            >
              Download State Snapshot
            </button>
          )}
        </div>
      </div>

      {result.error ? (
        <div
          style={{
            backgroundColor: '#0d1117',
            padding: '16px',
            borderRadius: '6px',
            marginBottom: '12px',
            fontSize: '13px',
            border: '1px solid #fb8500',
          }}
        >
          <div style={{ marginBottom: '12px' }}>
            <div style={{ color: '#fb8500', fontWeight: '600', marginBottom: '8px', display: 'flex', alignItems: 'center', gap: '8px' }}>
              Error Details
              {result.errorType && (
                <span
                  style={{
                    fontSize: '11px',
                    backgroundColor: '#2d1810',
                    color: '#f0883e',
                    padding: '2px 8px',
                    borderRadius: '3px',
                    border: '1px solid #fb8500',
                    fontFamily: 'monospace',
                    fontWeight: 'normal',
                  }}
                >
                  {result.errorType}
                </span>
              )}
            </div>
            <div
              style={{
                backgroundColor: '#1a1f26',
                padding: '12px',
                borderRadius: '4px',
                color: '#f0883e',
                fontFamily: 'monospace',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
                border: '1px solid #30363d',
              }}
            >
              {result.error}
            </div>
          </div>
          <div style={{ fontSize: '12px', color: '#8b949e', lineHeight: 1.6 }}>
            {result.errorType === 'NETWORK_ERROR' ? (
              <>
                ⚠️ The analyzer backend isn’t responding — it may have crashed or isn’t running.
                <br />
                Start it with <code style={{ color: '#00d9ff' }}>cargo run</code> (expected at{' '}
                <code style={{ color: '#00d9ff' }}>localhost:8080</code>), then retry.
              </>
            ) : result.errorType === 'PARSE_ERROR' ? (
              <>
                ⚠️ The backend returned a malformed response — it may have crashed mid-analysis.
                Check the analyzer logs, then retry.
              </>
            ) : result.errorType === 'INTERNAL_SERVER_ERROR' ? (
              <>💡 The analyzer hit an internal error during simulation. Check the analyzer logs for the panic trace.</>
            ) : (
              <>💡 Tip: Check if the backend is running and all parameters are correct.</>
            )}
          </div>
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
              maxHeight: '200px',
              overflow: 'auto',
            }}
          >
            <strong style={{ color: '#8b949e' }}>Result:</strong>
            <br />
            {JSON.stringify(result.result, null, 2)}
          </div>
        )
      )}

      {result.callGraphMermaid && (
        <CallGraphVisualizer mermaidDefinition={result.callGraphMermaid} />
      )}
    </div>
  );
}
