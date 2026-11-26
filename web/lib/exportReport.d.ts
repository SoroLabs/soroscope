import type { jsPDF } from 'jspdf';
import type { InvocationResult } from './sorobantypes';

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

export declare function generateReportSignature(
  result: InvocationResult,
  metadata?: ExportReportMetadata
): string;

export declare function extractGasBreakdownRows(
  result: InvocationResult
): GasMetricRow[];

export declare function generateCsvReport(
  result: InvocationResult,
  metadata?: ExportReportMetadata
): string;

export declare function generatePdfReport(
  result: InvocationResult,
  metadata?: ExportReportMetadata
): jsPDF;

export declare function downloadBlob(blob: Blob, filename: string): void;

export declare function exportReportToPdf(
  result: InvocationResult,
  metadata?: ExportReportMetadata,
  filename?: string
): jsPDF;

export declare function exportReportToCsv(
  result: InvocationResult,
  metadata?: ExportReportMetadata,
  filename?: string
): string;

export declare function exportReport(
  result: InvocationResult,
  format?: 'pdf' | 'csv' | 'both',
  metadata?: ExportReportMetadata
): { pdf?: jsPDF; csv?: string };
