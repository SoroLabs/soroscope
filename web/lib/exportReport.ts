/**
 * Client-Side Profiling Summary Report Exporter
 * Supports PDF (via jsPDF) and CSV export formats.
 *
 * Implements Issue #71
 */

import { jsPDF } from 'jspdf';
import type { InvocationResult, ResourceCost, ResourceReport } from './sorobantypes';

export interface ExportReportMetadata {
  contractId?: string;
  functionName?: string;
  timestamp?: number | string | Date;
  signature?: string;
  network?: string;
  wasmHash?: string;
  protocolVersion?: number;
  signerAddress?: string;
}

export interface GasMetricRow {
  metric: string;
  value: number | string;
  unit: string;
  testnetAverage?: number | string;
  difference?: string;
}

/**
 * Computes a deterministic report signature / hash from simulation results and metadata.
 */
export function generateReportSignature(
  result: InvocationResult,
  metadata?: ExportReportMetadata
): string {
  if (metadata?.signature) {
    return metadata.signature;
  }

  const payload = JSON.stringify({
    contractId: metadata?.contractId ?? 'unknown-contract',
    functionName: result.functionName,
    inputs: result.inputs,
    timestamp: result.timestamp,
    success: result.success,
    resourceCost: result.resourceCost,
  });

  let hash = 0;
  for (let i = 0; i < payload.length; i++) {
    const char = payload.charCodeAt(i);
    hash = ((hash << 5) - hash + char) | 0;
  }

  const hexHash = Math.abs(hash).toString(16).padStart(8, '0').toUpperCase();
  const signer = metadata?.signerAddress ? metadata.signerAddress.slice(0, 8).toUpperCase() : 'SOROSCOPE';
  const tsHex = result.timestamp.toString(16).toUpperCase();
  return `SIG-${signer}-${hexHash}-${tsHex}`;
}

/**
 * Normalizes resource breakdown metrics into structured rows.
 */
export function extractGasBreakdownRows(
  result: InvocationResult
): GasMetricRow[] {
  const rc = Object.assign(
    {},
    result.resourceCost,
    result.analysisReport
  ) as (ResourceCost & ResourceReport);
  if (!rc || Object.keys(rc).length === 0) return [];

  const averages =
    (result.resourceCost && result.resourceCost.testnet_averages) ||
    (result.analysisReport && (result.analysisReport as any).testnet_averages);

  const rows: GasMetricRow[] = [
    {
      metric: 'CPU Instructions',
      value: rc.cpu_instructions ?? 0,
      unit: 'instructions',
      testnetAverage: averages?.cpu_instructions,
      difference:
        averages?.cpu_instructions != null && rc.cpu_instructions != null
          ? `${rc.cpu_instructions > averages.cpu_instructions ? '+' : ''}${
              rc.cpu_instructions - averages.cpu_instructions
            }`
          : undefined,
    },
    {
      metric: 'RAM (Memory)',
      value: rc.ram_bytes ?? 0,
      unit: 'bytes',
      testnetAverage: averages?.ram_bytes,
      difference:
        averages?.ram_bytes != null && rc.ram_bytes != null
          ? `${rc.ram_bytes > averages.ram_bytes ? '+' : ''}${
              rc.ram_bytes - averages.ram_bytes
            }`
          : undefined,
    },
    {
      metric: 'Ledger Read Bytes',
      value: rc.ledger_read_bytes ?? 0,
      unit: 'bytes',
      testnetAverage: averages?.ledger_read_bytes,
      difference:
        averages?.ledger_read_bytes != null && rc.ledger_read_bytes != null
          ? `${rc.ledger_read_bytes > averages.ledger_read_bytes ? '+' : ''}${
              rc.ledger_read_bytes - averages.ledger_read_bytes
            }`
          : undefined,
    },
    {
      metric: 'Ledger Write Bytes',
      value: rc.ledger_write_bytes ?? 0,
      unit: 'bytes',
      testnetAverage: averages?.ledger_write_bytes,
      difference:
        averages?.ledger_write_bytes != null && rc.ledger_write_bytes != null
          ? `${rc.ledger_write_bytes > averages.ledger_write_bytes ? '+' : ''}${
              rc.ledger_write_bytes - averages.ledger_write_bytes
            }`
          : undefined,
    },
    {
      metric: 'Transaction Size',
      value: rc.transaction_size_bytes ?? 0,
      unit: 'bytes',
      testnetAverage: averages?.transaction_size_bytes,
      difference:
        averages?.transaction_size_bytes != null && rc.transaction_size_bytes != null
          ? `${rc.transaction_size_bytes > averages.transaction_size_bytes ? '+' : ''}${
              rc.transaction_size_bytes - averages.transaction_size_bytes
            }`
          : undefined,
    },
  ];

  if (rc.cost_stroops != null) {
    const xlmCost = (rc.cost_stroops / 10_000_000).toFixed(7);
    rows.push({
      metric: 'Cost (Stroops)',
      value: rc.cost_stroops,
      unit: 'stroops',
    });
    rows.push({
      metric: 'Estimated Fee (XLM)',
      value: xlmCost,
      unit: 'XLM',
    });
  } else if (rc.fee) {
    rows.push({
      metric: 'Estimated Fee',
      value: rc.fee,
      unit: 'XLM',
    });
  }

  return rows;
}

function escapeCsv(val: any): string {
  if (val === null || val === undefined) return '';
  const str = String(val);
  if (str.includes(',') || str.includes('"') || str.includes('\n') || str.includes('\r')) {
    return `"${str.replace(/"/g, '""')}"`;
  }
  return str;
}

/**
 * Generates raw CSV data containing metadata and gas breakdown tables.
 */
export function generateCsvReport(
  result: InvocationResult,
  metadata?: ExportReportMetadata
): string {
  const timestampIso = new Date(result.timestamp).toISOString();
  const signature = generateReportSignature(result, metadata);
  const contractId = metadata?.contractId ?? 'N/A';
  const protocolVersion =
    metadata?.protocolVersion ?? result.analysisReport?.protocol_version ?? 'N/A';

  const lines: string[] = [];

  // Metadata Section
  lines.push('SOROSCOPE PROFILING SUMMARY REPORT');
  lines.push('--- CONTRACT METADATA ---');
  lines.push(`Contract ID,${escapeCsv(contractId)}`);
  lines.push(`Function Name,${escapeCsv(result.functionName)}`);
  lines.push(`Execution Status,${result.success ? 'SUCCESS' : 'ERROR'}`);
  lines.push(`Timestamp (ISO),${escapeCsv(timestampIso)}`);
  lines.push(`Timestamp (Epoch ms),${result.timestamp}`);
  lines.push(`Protocol Version,${escapeCsv(protocolVersion)}`);
  if (metadata?.network) lines.push(`Network,${escapeCsv(metadata.network)}`);
  if (metadata?.signerAddress) lines.push(`Signer / Wallet,${escapeCsv(metadata.signerAddress)}`);
  lines.push(`Report Signature,${escapeCsv(signature)}`);
  lines.push('');

  // Gas Breakdown Data Table
  lines.push('--- GAS & RESOURCE BREAKDOWN RAW DATA ---');
  lines.push('Metric,Value,Unit,Testnet Average,Difference');
  const metrics = extractGasBreakdownRows(result);
  for (const m of metrics) {
    lines.push(
      [
        escapeCsv(m.metric),
        escapeCsv(m.value),
        escapeCsv(m.unit),
        escapeCsv(m.testnetAverage ?? ''),
        escapeCsv(m.difference ?? ''),
      ].join(',')
    );
  }
  lines.push('');

  // Inputs Section
  lines.push('--- FUNCTION INPUTS ---');
  lines.push('Parameter,Value');
  const inputs = result.inputs || {};
  const inputEntries = Object.entries(inputs);
  if (inputEntries.length === 0) {
    lines.push('(none),');
  } else {
    for (const [k, v] of inputEntries) {
      lines.push(`${escapeCsv(k)},${escapeCsv(typeof v === 'object' ? JSON.stringify(v) : v)}`);
    }
  }
  lines.push('');

  // Execution Result or Error
  lines.push('--- SIMULATION RESULT ---');
  if (result.success) {
    lines.push(`Status,SUCCESS`);
    lines.push(`Result Payload,${escapeCsv(JSON.stringify(result.result ?? null))}`);
  } else {
    lines.push(`Status,ERROR`);
    lines.push(`Error Type,${escapeCsv(result.errorType || 'UNKNOWN_ERROR')}`);
    lines.push(`Error Message,${escapeCsv(result.error || '')}`);
  }
  lines.push('');

  // Extended TTL Touched Entries
  if (result.analysisReport?.ttl_analysis?.touched_entries?.length) {
    lines.push('--- TTL TOUCHED ENTRIES ---');
    lines.push('Key,Live Until Ledger,Remaining Ledgers');
    for (const t of result.analysisReport.ttl_analysis.touched_entries) {
      lines.push(`${escapeCsv(t.key)},${t.live_until_ledger},${t.remaining_ledgers}`);
    }
    lines.push('');
  }

  // Extended State Dependencies
  if (result.analysisReport?.state_dependency?.length) {
    lines.push('--- STATE DEPENDENCIES ---');
    lines.push('Key,Source');
    for (const s of result.analysisReport.state_dependency) {
      lines.push(`${escapeCsv(s.key)},${escapeCsv(s.source)}`);
    }
    lines.push('');
  }

  // Nutrition & Efficiency Insights
  if (result.analysisReport?.nutrition?.insights?.length) {
    lines.push('--- OPTIMIZATION INSIGHTS ---');
    lines.push('Severity,Rule,Message,Suggested Fix');
    for (const ins of result.analysisReport.nutrition.insights) {
      lines.push(
        [
          escapeCsv(ins.severity),
          escapeCsv(ins.rule),
          escapeCsv(ins.message),
          escapeCsv(ins.suggested_fix),
        ].join(',')
      );
    }
    lines.push('');
  }

  return lines.join('\r\n');
}

/**
 * Generates a formatted PDF document using jsPDF with styling, metadata, and gas breakdown tables.
 */
export function generatePdfReport(
  result: InvocationResult,
  metadata?: ExportReportMetadata
): jsPDF {
  const doc = new jsPDF({
    orientation: 'portrait',
    unit: 'mm',
    format: 'a4',
  });

  const pageWidth = doc.internal.pageSize.getWidth();
  const pageHeight = doc.internal.pageSize.getHeight();
  const margin = 14;
  const contentWidth = pageWidth - margin * 2;

  let y = 14;

  const ensureSpace = (neededHeight: number) => {
    if (y + neededHeight > pageHeight - 16) {
      doc.addPage();
      y = 16;
    }
  };

  // Header Banner
  doc.setFillColor(15, 23, 42); // slate-900
  doc.rect(0, 0, pageWidth, 28, 'F');

  // Accent Line
  doc.setFillColor(0, 217, 255); // #00d9ff cyan
  doc.rect(0, 28, pageWidth, 1.5, 'F');

  // Header text
  doc.setTextColor(255, 255, 255);
  doc.setFont('helvetica', 'bold');
  doc.setFontSize(16);
  doc.text('SOROSCOPE PROFILER', margin, 14);

  doc.setFont('helvetica', 'normal');
  doc.setFontSize(10);
  doc.setTextColor(148, 163, 184); // slate-400
  doc.text('Smart Contract Execution & Gas Breakdown Summary Report', margin, 21);

  const reportDateStr = new Date(result.timestamp).toUTCString();
  doc.setFontSize(8);
  doc.setTextColor(203, 213, 225);
  doc.text(`Generated: ${reportDateStr}`, pageWidth - margin, 14, { align: 'right' });

  y = 38;

  // Metadata Card Block
  doc.setFillColor(248, 250, 252); // slate-50
  doc.setDrawColor(226, 232, 240); // slate-200
  doc.roundedRect(margin, y, contentWidth, 34, 2, 2, 'FD');

  const contractId = metadata?.contractId ?? 'CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q';
  const signature = generateReportSignature(result, metadata);
  const protocolVersion =
    metadata?.protocolVersion ?? result.analysisReport?.protocol_version ?? '20';

  doc.setFont('helvetica', 'bold');
  doc.setFontSize(11);
  doc.setTextColor(30, 41, 59);
  doc.text('Contract Metadata', margin + 4, y + 6);

  // Status Badge
  const statusColor = result.success ? [16, 185, 129] : [239, 68, 68]; // emerald-500 : red-500
  doc.setFillColor(statusColor[0], statusColor[1], statusColor[2]);
  doc.roundedRect(pageWidth - margin - 32, y + 3, 28, 6, 1, 1, 'F');
  doc.setTextColor(255, 255, 255);
  doc.setFontSize(8);
  doc.setFont('helvetica', 'bold');
  doc.text(result.success ? 'SUCCESS' : 'ERROR', pageWidth - margin - 18, y + 7.2, {
    align: 'center',
  });

  doc.setFont('helvetica', 'normal');
  doc.setFontSize(9);
  doc.setTextColor(71, 85, 105);

  doc.text(`Contract ID: ${contractId}`, margin + 4, y + 13);
  doc.text(`Function: ${result.functionName}()`, margin + 4, y + 18);
  doc.text(`Protocol Version: ${protocolVersion}`, margin + 4, y + 23);
  if (metadata?.network) {
    doc.text(`Network: ${metadata.network}`, margin + 90, y + 18);
  }
  doc.setFontSize(8);
  doc.setTextColor(100, 116, 139);
  doc.text(`Report Signature: ${signature}`, margin + 4, y + 29);

  y += 42;

  // Gas & Resource Breakdown Section
  ensureSpace(40);
  doc.setFont('helvetica', 'bold');
  doc.setFontSize(12);
  doc.setTextColor(15, 23, 42);
  doc.text('Gas & Resource Consumption Breakdown', margin, y);
  y += 4;

  // Table Header
  doc.setFillColor(30, 41, 59); // slate-800
  doc.rect(margin, y, contentWidth, 7, 'F');
  doc.setTextColor(255, 255, 255);
  doc.setFontSize(8.5);
  doc.setFont('helvetica', 'bold');

  const col1 = margin + 3;
  const col2 = margin + 65;
  const col3 = margin + 105;
  const col4 = margin + 140;

  doc.text('Metric', col1, y + 4.8);
  doc.text('Value', col2, y + 4.8);
  doc.text('Unit', col3, y + 4.8);
  doc.text('Testnet Avg', col4, y + 4.8);
  y += 7;

  const gasRows = extractGasBreakdownRows(result);
  doc.setFont('helvetica', 'normal');
  doc.setFontSize(8.5);

  gasRows.forEach((row, idx) => {
    ensureSpace(6);
    if (idx % 2 === 0) {
      doc.setFillColor(248, 250, 252); // subtle alternating row
      doc.rect(margin, y, contentWidth, 6, 'F');
    }
    doc.setDrawColor(241, 245, 249);
    doc.line(margin, y + 6, margin + contentWidth, y + 6);

    doc.setTextColor(30, 41, 59);
    doc.text(row.metric, col1, y + 4.2);

    const formattedVal =
      typeof row.value === 'number' ? row.value.toLocaleString() : String(row.value);
    doc.text(formattedVal, col2, y + 4.2);
    doc.setTextColor(100, 116, 139);
    doc.text(row.unit, col3, y + 4.2);

    const avgText =
      row.testnetAverage != null
        ? `${typeof row.testnetAverage === 'number' ? row.testnetAverage.toLocaleString() : row.testnetAverage} (${row.difference || '0'})`
        : '—';
    doc.text(avgText, col4, y + 4.2);

    y += 6;
  });

  y += 6;

  // Simulation Inputs & Output
  ensureSpace(35);
  doc.setFont('helvetica', 'bold');
  doc.setFontSize(11);
  doc.setTextColor(15, 23, 42);
  doc.text('Simulation Details & Parameters', margin, y);
  y += 4;

  doc.setFillColor(248, 250, 252);
  doc.setDrawColor(226, 232, 240);
  doc.roundedRect(margin, y, contentWidth, 26, 1.5, 1.5, 'FD');

  doc.setFont('helvetica', 'bold');
  doc.setFontSize(8.5);
  doc.setTextColor(71, 85, 105);
  doc.text('Input Arguments:', margin + 3, y + 5);

  doc.setFont('helvetica', 'normal');
  doc.setTextColor(15, 23, 42);
  const inputEntries = Object.entries(result.inputs || {});
  const inputSummary =
    inputEntries.length > 0
      ? inputEntries.map(([k, v]) => `${k}: ${typeof v === 'object' ? JSON.stringify(v) : v}`).join('; ')
      : '(none)';
  const wrappedInputs = doc.splitTextToSize(inputSummary, contentWidth - 8);
  doc.text(wrappedInputs, margin + 3, y + 9);

  doc.setFont('helvetica', 'bold');
  doc.setTextColor(71, 85, 105);
  doc.text('Result Output:', margin + 3, y + 17);

  doc.setFont('helvetica', 'normal');
  if (result.success) {
    doc.setTextColor(16, 185, 129);
    const resultSummary = JSON.stringify(result.result ?? 'Success', null, 2);
    const wrappedResult = doc.splitTextToSize(resultSummary, contentWidth - 8);
    doc.text(wrappedResult, margin + 3, y + 21);
  } else {
    doc.setTextColor(239, 68, 68);
    const errText = `${result.errorType ? `[${result.errorType}] ` : ''}${result.error || 'Execution failed'}`;
    const wrappedErr = doc.splitTextToSize(errText, contentWidth - 8);
    doc.text(wrappedErr, margin + 3, y + 21);
  }

  y += 32;

  // Extended Insights if available
  if (result.analysisReport?.nutrition?.insights?.length) {
    ensureSpace(30);
    doc.setFont('helvetica', 'bold');
    doc.setFontSize(11);
    doc.setTextColor(15, 23, 42);
    doc.text('Optimization & Gas Golfing Insights', margin, y);
    y += 5;

    for (const insight of result.analysisReport.nutrition.insights) {
      ensureSpace(14);
      doc.setFillColor(254, 242, 242); // light red/amber
      doc.setDrawColor(254, 202, 202);
      doc.roundedRect(margin, y, contentWidth, 12, 1, 1, 'FD');

      doc.setFont('helvetica', 'bold');
      doc.setFontSize(8);
      doc.setTextColor(185, 28, 28);
      doc.text(`[${insight.severity.toUpperCase()}] ${insight.rule}`, margin + 3, y + 4.5);

      doc.setFont('helvetica', 'normal');
      doc.setTextColor(30, 41, 59);
      doc.text(insight.message, margin + 3, y + 8);
      y += 14;
    }
  }

  // Footer on all pages
  const totalPages = doc.getNumberOfPages();
  for (let i = 1; i <= totalPages; i++) {
    doc.setPage(i);
    doc.setDrawColor(226, 232, 240);
    doc.line(margin, pageHeight - 12, pageWidth - margin, pageHeight - 12);

    doc.setFontSize(7.5);
    doc.setFont('helvetica', 'normal');
    doc.setTextColor(148, 163, 184);
    doc.text('Soroscope Profiler · Stellar / Soroban Contract Analytics', margin, pageHeight - 7);
    doc.text(`Signature: ${signature}  |  Page ${i} of ${totalPages}`, pageWidth - margin, pageHeight - 7, {
      align: 'right',
    });
  }

  return doc;
}

/**
 * Downloads a Blob client-side.
 */
export function downloadBlob(blob: Blob, filename: string): void {
  if (typeof window === 'undefined') return;
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

/**
 * Exports full simulation details to formatted PDF document and triggers download.
 */
export function exportReportToPdf(
  result: InvocationResult,
  metadata?: ExportReportMetadata,
  filename?: string
): jsPDF {
  const doc = generatePdfReport(result, metadata);
  const finalFilename =
    filename || `soroscope-profiling-report-${result.functionName || 'function'}-${Date.now()}.pdf`;
  if (typeof window !== 'undefined') {
    doc.save(finalFilename);
  }
  return doc;
}

/**
 * Exports gas breakdown raw data tables to CSV format and triggers download.
 */
export function exportReportToCsv(
  result: InvocationResult,
  metadata?: ExportReportMetadata,
  filename?: string
): string {
  const csvContent = generateCsvReport(result, metadata);
  const finalFilename =
    filename || `soroscope-gas-breakdown-report-${result.functionName || 'function'}-${Date.now()}.csv`;
  if (typeof window !== 'undefined') {
    const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
    downloadBlob(blob, finalFilename);
  }
  return csvContent;
}

/**
 * Unified export report utility supporting PDF, CSV, or both.
 */
export function exportReport(
  result: InvocationResult,
  format: 'pdf' | 'csv' | 'both' = 'pdf',
  metadata?: ExportReportMetadata
): { pdf?: jsPDF; csv?: string } {
  const output: { pdf?: jsPDF; csv?: string } = {};

  if (format === 'pdf' || format === 'both') {
    output.pdf = exportReportToPdf(result, metadata);
  }

  if (format === 'csv' || format === 'both') {
    output.csv = exportReportToCsv(result, metadata);
  }

  return output;
}
