import { useState } from 'react';
import type { InvocationResult } from '../lib/sorobantypes';
import { exportReport } from '../lib/exportReport';

import { CallGraphVisualizer } from './CallGraphVisualizer';
import { CopyButton } from './CopyButton';

interface ResultViewerProps {
  result: InvocationResult | null;
  contractId?: string;
}

export function ResultViewer({ result, contractId }: ResultViewerProps) {
  const [exportFormat, setExportFormat] = useState<'pdf' | 'csv' | 'both'>('pdf');
  const [exporting, setExporting] = useState(false);

  const handleExport = (format: 'pdf' | 'csv' | 'both' = exportFormat) => {
    if (!result) return;
    setExporting(true);
    try {
      exportReport(result, format, {
        contractId,
        functionName: result.functionName,
      });
    } catch (err) {
      console.error('Failed to export report:', err);
    } finally {
      setExporting(false);
    }
  };

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

  if (!result) {
    return (
      <div
        style={{
          padding: '24px',
          backgroundColor: 'var(--bg-elevated)',
          borderRadius: '8px',
          textAlign: 'center',
          color: 'var(--text-secondary)',
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
        backgroundColor: 'var(--bg-elevated)',
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
          <p style={{ margin: '0', color: 'var(--text-secondary)', fontSize: '12px' }}>
            {new Date(result.timestamp).toLocaleString()}
          </p>
        </div>
        
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
          {/* Export Report Group */}
          <div style={{ display: 'inline-flex', borderRadius: '6px', overflow: 'hidden', border: '1px solid #374151' }}>
            <button
              id="export-report-button"
              onClick={() => handleExport(exportFormat)}
              disabled={exporting}
              style={{
                padding: '6px 12px',
                backgroundColor: '#0284c7',
                color: '#ffffff',
                border: 'none',
                fontSize: '12px',
                fontWeight: '500',
                cursor: exporting ? 'wait' : 'pointer',
                transition: 'background-color 0.2s',
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
              }}
              onMouseOver={(e) => (e.currentTarget.style.backgroundColor = '#0369a1')}
              onMouseOut={(e) => (e.currentTarget.style.backgroundColor = '#0284c7')}
              title={`Export profiling summary report as ${exportFormat.toUpperCase()}`}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
              {exporting ? 'Exporting...' : 'Export Report'}
            </button>
            <select
              value={exportFormat}
              onChange={(e) => setExportFormat(e.target.value as 'pdf' | 'csv' | 'both')}
              aria-label="Export format"
              style={{
                backgroundColor: '#1f2937',
                color: '#f3f4f6',
                border: 'none',
                borderLeft: '1px solid #374151',
                padding: '6px 8px',
                fontSize: '11px',
                cursor: 'pointer',
                outline: 'none',
              }}
            >
              <option value="pdf">PDF</option>
              <option value="csv">CSV</option>
              <option value="both">Both (PDF & CSV)</option>
            </select>
          </div>

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
              Snapshot JSON
            </button>
          )}
        </div>
      </div>

      {result.error ? (
        <div
          style={{
            backgroundColor: 'var(--bg-elevated)',
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
          <div style={{ fontSize: '12px', color: 'var(--text-secondary)', lineHeight: 1.6 }}>
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
              backgroundColor: 'var(--bg-elevated)',
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
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
              <strong style={{ color: 'var(--text-secondary)' }}>Result:</strong>
              <CopyButton text={JSON.stringify(result.result, null, 2)} label="Copy Result" tooltipPosition="left" />
            </div>
            {JSON.stringify(result.result, null, 2)}
          </div>
        )
      )}

      {(result.callGraph || result.callGraphMermaid) && (
        <CallGraphVisualizer
          callGraph={result.callGraph}
          mermaidDefinition={result.callGraphMermaid}
        />
      )}
    </div>
  );
}
