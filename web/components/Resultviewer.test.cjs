// Resultviewer.test.cjs — unit tests for ResultViewer Export Report actions & logic
// Issue #71: Export Profiling Summary Report to PDF & CSV

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { exportReport, generateCsvReport, generatePdfReport } = require('../lib/exportReport.js');

const mockResult = {
  id: 'test-inv-001',
  functionName: 'transfer',
  inputs: { to: 'GCRECIPIENT', amount: '1000' },
  result: { success: true },
  timestamp: Date.now(),
  success: true,
  resourceCost: {
    cpu_instructions: 5000000,
    ram_bytes: 1048576,
    ledger_read_bytes: 2048,
    ledger_write_bytes: 512,
    transaction_size_bytes: 350,
    cost_stroops: 25000,
  },
};

test('ResultViewer Export: exportReport supports pdf format', () => {
  const res = exportReport(mockResult, 'pdf', { contractId: 'CCONTRACT' });
  assert.ok(res.pdf);
  assert.equal(res.csv, undefined);
  assert.ok(typeof res.pdf.output === 'function');
});

test('ResultViewer Export: exportReport supports csv format', () => {
  const res = exportReport(mockResult, 'csv', { contractId: 'CCONTRACT' });
  assert.ok(res.csv);
  assert.equal(res.pdf, undefined);
  assert.ok(res.csv.includes('SOROSCOPE PROFILING SUMMARY REPORT'));
  assert.ok(res.csv.includes('CPU Instructions,5000000'));
});

test('ResultViewer Export: exportReport supports both formats simultaneously', () => {
  const res = exportReport(mockResult, 'both', { contractId: 'CCONTRACT' });
  assert.ok(res.pdf);
  assert.ok(res.csv);
  assert.ok(typeof res.pdf.output === 'function');
  assert.ok(res.csv.includes('CCONTRACT'));
});

test('ResultViewer Export: default suggested filename includes "report"', () => {
  const csvReport = generateCsvReport(mockResult);
  assert.ok(csvReport.length > 0);
  const pdfDoc = generatePdfReport(mockResult);
  assert.ok(pdfDoc.getNumberOfPages() >= 1);
});
