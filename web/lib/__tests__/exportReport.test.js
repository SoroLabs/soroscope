// exportReport.test.js - Unit tests for exportReport utility
// Issue #71: Export Profiling Summary Report to PDF & CSV

const test = require('node:test');
const assert = require('node:assert/strict');
const { jsPDF } = require('jspdf');

// Import transpiled or raw JS functions
// We can define the test logic matching lib/exportReport.ts
const {
  generateReportSignature,
  extractGasBreakdownRows,
  generateCsvReport,
  generatePdfReport,
} = require('../exportReport.js');

const mockSuccessResult = {
  id: 'test-inv-123',
  functionName: 'transfer',
  inputs: {
    from: 'GBEXAMPLEUSER1234567890',
    to: 'GCRECIPIENT9876543210',
    amount: '5000000',
  },
  result: { success: true, tx_hash: '0x123abc' },
  timestamp: 1775000000000,
  success: true,
  resourceCost: {
    cpu_instructions: 14500000,
    ram_bytes: 2097152,
    ledger_read_bytes: 4096,
    ledger_write_bytes: 1024,
    transaction_size_bytes: 640,
    cost_stroops: 50000,
    testnet_averages: {
      cpu_instructions: 12000000,
      ram_bytes: 1048576,
      ledger_read_bytes: 2048,
      ledger_write_bytes: 1024,
      transaction_size_bytes: 800,
    },
  },
  analysisReport: {
    protocol_version: 21,
    cost_stroops: 50000,
    cpu_instructions: 14500000,
    ram_bytes: 2097152,
    ledger_read_bytes: 4096,
    ledger_write_bytes: 1024,
    transaction_size_bytes: 640,
    state_dependency: [
      { key: 'balance:GBEXAMPLE', source: 'Live' },
    ],
    ttl_analysis: {
      current_ledger: 100000,
      touched_entries: [
        { key: 'balance:GBEXAMPLE', live_until_ledger: 120000, remaining_ledgers: 20000 },
      ],
      extend_ttl_suggestions: [],
    },
    nutrition: {
      efficiency_score: 92,
      insights: [
        {
          severity: 'info',
          rule: 'cpu-instructions',
          message: 'CPU usage is within normal bounds',
          suggested_fix: 'None needed',
        },
      ],
    },
  },
};

const mockErrorResult = {
  id: 'test-err-456',
  functionName: 'mint',
  inputs: {
    to: 'GCRECIPIENT9876543210',
    amount: '100000000',
  },
  error: 'HostError: Error(Contract, #1) Unauthorized',
  errorType: 'BAD_REQUEST',
  timestamp: 1775000000000,
  success: false,
  resourceCost: {
    cpu_instructions: 2500000,
    ram_bytes: 512000,
    ledger_read_bytes: 1024,
    ledger_write_bytes: 0,
    transaction_size_bytes: 320,
    cost_stroops: 12000,
  },
};

test('generateReportSignature produces deterministic signature', () => {
  const sig1 = generateReportSignature(mockSuccessResult, { contractId: 'CAEZJVJ4' });
  const sig2 = generateReportSignature(mockSuccessResult, { contractId: 'CAEZJVJ4' });
  assert.equal(sig1, sig2);
  assert.ok(sig1.startsWith('SIG-SOROSCOPE-'));
});

test('generateReportSignature respects custom signature in metadata', () => {
  const customSig = 'CUSTOM-SIGNATURE-xyz';
  const sig = generateReportSignature(mockSuccessResult, { signature: customSig });
  assert.equal(sig, customSig);
});

test('extractGasBreakdownRows computes correct metrics and differences', () => {
  const rows = extractGasBreakdownRows(mockSuccessResult);
  assert.ok(rows.length >= 5);

  const cpuRow = rows.find((r) => r.metric === 'CPU Instructions');
  assert.ok(cpuRow);
  assert.equal(cpuRow.value, 14500000);
  assert.equal(cpuRow.unit, 'instructions');
  assert.equal(cpuRow.testnetAverage, 12000000);
  assert.equal(cpuRow.difference, '+2500000');

  const ramRow = rows.find((r) => r.metric === 'RAM (Memory)');
  assert.ok(ramRow);
  assert.equal(ramRow.value, 2097152);

  const feeRow = rows.find((r) => r.metric.includes('Fee') || r.metric.includes('Cost'));
  assert.ok(feeRow);
});

test('generateCsvReport includes metadata, timestamp, signature, and gas breakdown', () => {
  const csv = generateCsvReport(mockSuccessResult, {
    contractId: 'CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q',
    network: 'Testnet',
  });

  // Verify headers and metadata
  assert.ok(csv.includes('SOROSCOPE PROFILING SUMMARY REPORT'));
  assert.ok(csv.includes('Contract ID,CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q'));
  assert.ok(csv.includes('Function Name,transfer'));
  assert.ok(csv.includes('Execution Status,SUCCESS'));
  assert.ok(csv.includes('Timestamp (ISO)'));
  assert.ok(csv.includes('Report Signature,SIG-'));
  assert.ok(csv.includes('Network,Testnet'));

  // Verify gas breakdown table
  assert.ok(csv.includes('--- GAS & RESOURCE BREAKDOWN RAW DATA ---'));
  assert.ok(csv.includes('CPU Instructions,14500000,instructions,12000000,+2500000'));
  assert.ok(csv.includes('RAM (Memory),2097152,bytes,1048576,+1048576'));

  // Verify inputs and result
  assert.ok(csv.includes('--- FUNCTION INPUTS ---'));
  assert.ok(csv.includes('from,GBEXAMPLEUSER1234567890'));
  assert.ok(csv.includes('--- SIMULATION RESULT ---'));
  assert.ok(csv.includes('Result Payload'));

  // Verify TTL and state dependencies
  assert.ok(csv.includes('--- TTL TOUCHED ENTRIES ---'));
  assert.ok(csv.includes('balance:GBEXAMPLE,120000,20000'));
});

test('generateCsvReport formats error simulations accurately', () => {
  const csv = generateCsvReport(mockErrorResult, {
    contractId: 'CAEZJVJ4N7P7',
  });

  assert.ok(csv.includes('Execution Status,ERROR'));
  assert.ok(csv.includes('Error Type,BAD_REQUEST'));
  assert.ok(csv.includes('Unauthorized'));
});

test('generatePdfReport creates valid jsPDF instance with metadata and gas breakdown', () => {
  const doc = generatePdfReport(mockSuccessResult, {
    contractId: 'CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q',
    network: 'Testnet',
  });

  assert.ok(typeof doc.output === 'function');
  assert.ok(typeof doc.save === 'function');
  assert.ok(doc.getNumberOfPages() >= 1);

  // Validate PDF output string/blob
  const output = doc.output();
  assert.ok(output.includes('%PDF-'));
  assert.ok(output.length > 500);
});

test('generatePdfReport handles error result gracefully', () => {
  const doc = generatePdfReport(mockErrorResult);
  assert.ok(typeof doc.output === 'function');
  assert.ok(typeof doc.save === 'function');
  assert.ok(doc.getNumberOfPages() >= 1);
  const output = doc.output();
  assert.ok(output.includes('%PDF-'));
});
