/**
 * BatchImportPanel — upload a CSV or JSON file of contract call definitions,
 * run them through the analyzeService, and display a results table.
 *
 * Closes #1294.
 */

import React, { useCallback, useRef, useState } from 'react';
import { Upload, X, AlertTriangle, CheckCircle2, XCircle, Loader2, FileText, Download } from 'lucide-react';
import { parseBatchFile, type BatchCallDefinition } from '../lib/batchImport';
import { analyzeService } from '../lib/api';
import type { AnalyzeResponse } from '../lib/sorobantypes';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type RowStatus = 'pending' | 'running' | 'success' | 'error';

interface BatchRow {
  id: string;
  call: BatchCallDefinition;
  status: RowStatus;
  report?: AnalyzeResponse;
  error?: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatNumber(n: number | undefined): string {
  if (n === undefined || n === null) return '—';
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function formatFee(stroops: number | undefined): string {
  if (stroops === undefined || stroops === null) return '—';
  // 1 XLM = 10,000,000 stroops
  const xlm = stroops / 10_000_000;
  return xlm < 0.001 ? `${stroops} str` : `${xlm.toFixed(4)} XLM`;
}

function downloadCsv(rows: BatchRow[]): void {
  const header = ['contract_id', 'function_name', 'status', 'cpu_instructions', 'ram_bytes', 'fee', 'error'];
  const body = rows.map((r) => [
    r.call.contract_id,
    r.call.function_name,
    r.status,
    r.report?.cpu_instructions ?? '',
    r.report?.ram_bytes ?? '',
    r.report?.cost_stroops ?? '',
    r.error ?? '',
  ]);

  const csvContent = [header, ...body]
    .map((cols) =>
      cols.map((c) => (String(c).includes(',') || String(c).includes('"') ? `"${String(c).replace(/"/g, '""')}"` : c)).join(','),
    )
    .join('\n');

  const blob = new Blob([csvContent], { type: 'text/csv' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `soroscope-batch-${Date.now()}.csv`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function StatusBadge({ status }: { status: RowStatus }) {
  if (status === 'pending') {
    return (
      <span style={{ color: '#8b949e', fontSize: '12px' }}>Pending</span>
    );
  }
  if (status === 'running') {
    return (
      <span style={{ display: 'flex', alignItems: 'center', gap: '4px', color: '#58a6ff', fontSize: '12px' }}>
        <Loader2 size={12} style={{ animation: 'spin 1s linear infinite' }} />
        Running
      </span>
    );
  }
  if (status === 'success') {
    return (
      <span style={{ display: 'flex', alignItems: 'center', gap: '4px', color: '#3fb950', fontSize: '12px' }}>
        <CheckCircle2 size={12} />
        OK
      </span>
    );
  }
  return (
    <span style={{ display: 'flex', alignItems: 'center', gap: '4px', color: '#f85149', fontSize: '12px' }}>
      <XCircle size={12} />
      Error
    </span>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

interface BatchImportPanelProps {
  /** Auth token forwarded to analyzeService (optional). */
  token?: string;
}

export function BatchImportPanel({ token }: BatchImportPanelProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [rows, setRows] = useState<BatchRow[]>([]);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [parseError, setParseError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [fileName, setFileName] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);

  // -------------------------------------------------------------------
  // File ingestion
  // -------------------------------------------------------------------

  const processFile = useCallback(async (file: File) => {
    setParseError(null);
    setWarnings([]);
    setRows([]);
    setFileName(file.name);

    const ext = file.name.split('.').pop()?.toLowerCase();
    if (ext !== 'csv' && ext !== 'json') {
      setParseError('Unsupported file type. Please upload a .csv or .json file.');
      return;
    }

    let text: string;
    try {
      text = await file.text();
    } catch {
      setParseError('Could not read the file. Please try again.');
      return;
    }

    try {
      const result = parseBatchFile(text, ext as 'csv' | 'json');
      if (result.calls.length === 0) {
        setParseError('No valid call definitions found in the file.');
        return;
      }
      setWarnings(result.warnings);
      setRows(
        result.calls.map((call, i) => ({
          id: `batch-${i}-${Date.now()}`,
          call,
          status: 'pending',
        })),
      );
    } catch (e) {
      setParseError(e instanceof Error ? e.message : 'Failed to parse file.');
    }
  }, []);

  const handleFileInput = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (file) await processFile(file);
      // Reset input so the same file can be re-selected after a reset
      e.target.value = '';
    },
    [processFile],
  );

  const handleDrop = useCallback(
    async (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      setDragOver(false);
      const file = e.dataTransfer.files?.[0];
      if (file) await processFile(file);
    },
    [processFile],
  );

  // -------------------------------------------------------------------
  // Batch execution
  // -------------------------------------------------------------------

  const runBatch = useCallback(async () => {
    setRunning(true);

    for (let i = 0; i < rows.length; i++) {
      // Mark current row as running
      setRows((prev) =>
        prev.map((r, idx) => (idx === i ? { ...r, status: 'running' } : r)),
      );

      try {
        const { call } = rows[i];
        const report = await analyzeService.analyze(
          {
            contract_id: call.contract_id,
            function_name: call.function_name,
            args: call.args,
          },
          token,
        );

        setRows((prev) =>
          prev.map((r, idx) =>
            idx === i ? { ...r, status: 'success', report } : r,
          ),
        );
      } catch (err) {
        const message =
          err instanceof Error ? err.message : 'Analysis failed';
        setRows((prev) =>
          prev.map((r, idx) =>
            idx === i ? { ...r, status: 'error', error: message } : r,
          ),
        );
      }
    }

    setRunning(false);
  }, [rows, token]);

  const reset = useCallback(() => {
    setRows([]);
    setWarnings([]);
    setParseError(null);
    setFileName(null);
    setRunning(false);
  }, []);

  // -------------------------------------------------------------------
  // Derived stats
  // -------------------------------------------------------------------

  const successCount = rows.filter((r) => r.status === 'success').length;
  const errorCount = rows.filter((r) => r.status === 'error').length;
  const pendingCount = rows.filter((r) => r.status === 'pending').length;

  // -------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------

  return (
    <div
      style={{
        backgroundColor: '#161b22',
        borderRadius: '12px',
        padding: '24px',
        border: '1px solid #30363d',
        marginBottom: '24px',
      }}
    >
      {/* Header */}
      <div style={{ marginBottom: '20px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '12px' }}>
        <div>
          <h2 style={{ margin: '0 0 4px 0', fontSize: '16px', fontWeight: '600', color: '#c9d1d9', display: 'flex', alignItems: 'center', gap: '8px' }}>
            <Upload size={16} color="#58a6ff" />
            Batch Import
          </h2>
          <p style={{ margin: 0, fontSize: '13px', color: '#8b949e' }}>
            Upload a CSV or JSON file to analyze multiple contract calls at once.
          </p>
        </div>

        {rows.length > 0 && (
          <div style={{ display: 'flex', gap: '8px' }}>
            {!running && successCount + errorCount > 0 && (
              <button
                onClick={() => downloadCsv(rows)}
                style={{
                  display: 'flex', alignItems: 'center', gap: '6px',
                  padding: '7px 14px', borderRadius: '6px',
                  border: '1px solid #30363d', backgroundColor: '#21262d',
                  color: '#c9d1d9', fontSize: '13px', cursor: 'pointer',
                }}
                title="Export results as CSV"
              >
                <Download size={13} />
                Export CSV
              </button>
            )}
            <button
              onClick={reset}
              disabled={running}
              style={{
                display: 'flex', alignItems: 'center', gap: '6px',
                padding: '7px 14px', borderRadius: '6px',
                border: '1px solid #30363d', backgroundColor: '#21262d',
                color: '#8b949e', fontSize: '13px', cursor: running ? 'not-allowed' : 'pointer',
                opacity: running ? 0.5 : 1,
              }}
            >
              <X size={13} />
              Clear
            </button>
          </div>
        )}
      </div>

      {/* Upload area — shown when no file is loaded */}
      {rows.length === 0 && !parseError && (
        <div
          onDrop={handleDrop}
          onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
          onDragLeave={() => setDragOver(false)}
          onClick={() => fileInputRef.current?.click()}
          role="button"
          aria-label="Upload batch call definitions file"
          tabIndex={0}
          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') fileInputRef.current?.click(); }}
          style={{
            border: `2px dashed ${dragOver ? '#58a6ff' : '#30363d'}`,
            borderRadius: '8px',
            padding: '36px 24px',
            textAlign: 'center',
            cursor: 'pointer',
            backgroundColor: dragOver ? '#0d1117' : 'transparent',
            transition: 'border-color 0.2s, background-color 0.2s',
          }}
        >
          <FileText size={32} color="#8b949e" style={{ marginBottom: '12px' }} />
          <p style={{ margin: '0 0 8px 0', color: '#c9d1d9', fontSize: '14px', fontWeight: '500' }}>
            Drop a CSV or JSON file here, or click to browse
          </p>
          <p style={{ margin: 0, color: '#8b949e', fontSize: '12px' }}>
            CSV columns: <code style={{ color: '#58a6ff' }}>contract_id, function_name, args</code>
            &nbsp;·&nbsp;
            JSON: array of <code style={{ color: '#58a6ff' }}>{`{ contract_id, function_name, args? }`}</code>
          </p>
          <input
            ref={fileInputRef}
            type="file"
            accept=".csv,.json,text/csv,application/json"
            onChange={handleFileInput}
            style={{ display: 'none' }}
            aria-hidden="true"
          />
        </div>
      )}

      {/* Parse error */}
      {parseError && (
        <div
          style={{
            display: 'flex', alignItems: 'flex-start', gap: '10px',
            backgroundColor: '#2d1810', border: '1px solid #f85149',
            borderRadius: '8px', padding: '14px 16px', marginBottom: '16px',
          }}
        >
          <AlertTriangle size={16} color="#f85149" style={{ flexShrink: 0, marginTop: '1px' }} />
          <div>
            <p style={{ margin: '0 0 8px 0', color: '#f85149', fontWeight: '600', fontSize: '13px' }}>
              File parse error
            </p>
            <p style={{ margin: 0, color: '#f0883e', fontSize: '13px' }}>{parseError}</p>
          </div>
          <button
            onClick={reset}
            style={{ marginLeft: 'auto', background: 'none', border: 'none', cursor: 'pointer', color: '#8b949e', flexShrink: 0 }}
            aria-label="Dismiss error and reset"
          >
            <X size={14} />
          </button>
        </div>
      )}

      {/* Warnings */}
      {warnings.length > 0 && (
        <div
          style={{
            backgroundColor: '#1f1a10', border: '1px solid #d29922',
            borderRadius: '8px', padding: '12px 16px', marginBottom: '16px',
          }}
        >
          <p style={{ margin: '0 0 6px 0', color: '#d29922', fontWeight: '600', fontSize: '13px', display: 'flex', alignItems: 'center', gap: '6px' }}>
            <AlertTriangle size={14} />
            {warnings.length} warning{warnings.length > 1 ? 's' : ''}
          </p>
          <ul style={{ margin: 0, paddingLeft: '18px', color: '#e3b341', fontSize: '12px', display: 'flex', flexDirection: 'column', gap: '3px' }}>
            {warnings.map((w, i) => <li key={i}>{w}</li>)}
          </ul>
        </div>
      )}

      {/* File loaded — summary + run button */}
      {rows.length > 0 && (
        <>
          <div
            style={{
              display: 'flex', alignItems: 'center', justifyContent: 'space-between',
              flexWrap: 'wrap', gap: '12px', marginBottom: '16px',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
              {fileName && (
                <span style={{ color: '#58a6ff', fontSize: '13px', display: 'flex', alignItems: 'center', gap: '6px' }}>
                  <FileText size={13} />
                  {fileName}
                </span>
              )}
              <span style={{ color: '#8b949e', fontSize: '13px' }}>{rows.length} call{rows.length > 1 ? 's' : ''}</span>
              {successCount > 0 && <span style={{ color: '#3fb950', fontSize: '13px' }}>✓ {successCount} passed</span>}
              {errorCount > 0 && <span style={{ color: '#f85149', fontSize: '13px' }}>✗ {errorCount} failed</span>}
              {pendingCount > 0 && running && <span style={{ color: '#58a6ff', fontSize: '13px' }}>{pendingCount} pending</span>}
            </div>

            {!running && pendingCount === rows.length && (
              <button
                onClick={() => void runBatch()}
                style={{
                  display: 'flex', alignItems: 'center', gap: '8px',
                  padding: '8px 20px', borderRadius: '6px',
                  border: 'none', backgroundColor: '#238636',
                  color: '#ffffff', fontSize: '13px', fontWeight: '600',
                  cursor: 'pointer', transition: 'background-color 0.2s',
                }}
                onMouseOver={(e) => (e.currentTarget.style.backgroundColor = '#2ea043')}
                onMouseOut={(e) => (e.currentTarget.style.backgroundColor = '#238636')}
              >
                <Upload size={14} />
                Run {rows.length} Call{rows.length > 1 ? 's' : ''}
              </button>
            )}

            {running && (
              <span style={{ display: 'flex', alignItems: 'center', gap: '8px', color: '#58a6ff', fontSize: '13px' }}>
                <Loader2 size={14} style={{ animation: 'spin 1s linear infinite' }} />
                Running…
              </span>
            )}
          </div>

          {/* Results table */}
          <div style={{ overflowX: 'auto' }}>
            <table
              style={{
                width: '100%', borderCollapse: 'collapse',
                fontSize: '13px', color: '#c9d1d9',
              }}
              aria-label="Batch call results"
            >
              <thead>
                <tr style={{ borderBottom: '1px solid #30363d' }}>
                  {['#', 'Contract ID', 'Function', 'CPU', 'RAM', 'Fee', 'Status'].map((h) => (
                    <th
                      key={h}
                      style={{
                        padding: '8px 12px', textAlign: 'left',
                        color: '#8b949e', fontWeight: '600', fontSize: '12px',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {h}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((row, idx) => (
                  <tr
                    key={row.id}
                    style={{
                      borderBottom: '1px solid #21262d',
                      backgroundColor: row.status === 'running' ? '#0d2340' : 'transparent',
                    }}
                  >
                    <td style={{ padding: '8px 12px', color: '#8b949e' }}>{idx + 1}</td>
                    <td
                      style={{
                        padding: '8px 12px', fontFamily: 'monospace',
                        fontSize: '12px', maxWidth: '180px',
                        overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                      }}
                      title={row.call.contract_id}
                    >
                      {row.call.contract_id}
                    </td>
                    <td style={{ padding: '8px 12px', fontFamily: 'monospace', fontSize: '12px' }}>
                      {row.call.function_name}
                    </td>
                    <td style={{ padding: '8px 12px', textAlign: 'right', fontFamily: 'monospace' }}>
                      {formatNumber(row.report?.cpu_instructions)}
                    </td>
                    <td style={{ padding: '8px 12px', textAlign: 'right', fontFamily: 'monospace' }}>
                      {formatNumber(row.report?.ram_bytes)}
                    </td>
                    <td style={{ padding: '8px 12px', textAlign: 'right', fontFamily: 'monospace' }}>
                      {formatFee(row.report?.cost_stroops)}
                    </td>
                    <td style={{ padding: '8px 12px' }}>
                      {row.status === 'error' ? (
                        <span
                          title={row.error}
                          style={{ cursor: 'help' }}
                        >
                          <StatusBadge status={row.status} />
                        </span>
                      ) : (
                        <StatusBadge status={row.status} />
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      {/* Spinner keyframes injected once */}
      <style>{`
        @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
      `}</style>
    </div>
  );
}
