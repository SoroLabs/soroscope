'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildShareUrl,
  isValidSimulationId,
  simulationRecordToResult,
} = require('./shareLinks');

const UUID = '3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b';

test('buildShareUrl: uses soroscope.app/result/<id> by default', () => {
  assert.equal(buildShareUrl(UUID), `https://soroscope.app/result/${UUID}`);
});

test('buildShareUrl: honours a custom base and strips trailing slashes', () => {
  assert.equal(buildShareUrl('42', 'http://localhost:3000/'), 'http://localhost:3000/result/42');
  assert.equal(buildShareUrl('42', '  '), 'https://soroscope.app/result/42');
});

test('buildShareUrl: rejects ids that could escape the path', () => {
  for (const bad of ['', '../admin', 'a/b', 'a?b=1', 'x'.repeat(200), undefined]) {
    assert.equal(isValidSimulationId(bad), false, String(bad));
    assert.throws(() => buildShareUrl(bad));
  }
});

test('simulationRecordToResult: reads a nested report', () => {
  const { result, contractId } = simulationRecordToResult(
    {
      id: UUID,
      contract_id: 'CABC',
      function_name: 'transfer',
      args: ['GA', 5],
      created_at: '2026-10-20T12:00:00Z',
      report: { cpu_instructions: 10, ram_bytes: 20, state_snapshot: { latest_ledger: 1 } },
    },
    UUID,
  );
  assert.equal(contractId, 'CABC');
  assert.equal(result.simulationId, UUID);
  assert.equal(result.functionName, 'transfer');
  assert.deepEqual(result.inputs, { arg0: 'GA', arg1: '5' });
  assert.equal(result.timestamp, Date.parse('2026-10-20T12:00:00Z'));
  assert.equal(result.success, true);
  assert.equal(result.analysisReport.cpu_instructions, 10);
  assert.deepEqual(result.stateSnapshot, { latest_ledger: 1 });
});

test('simulationRecordToResult: reads a flattened report and epoch seconds', () => {
  const { result } = simulationRecordToResult(
    { simulation_id: '7', function_name: 'mint', cpu_instructions: 99, timestamp: 1_700_000_000 },
    '7',
  );
  assert.equal(result.analysisReport.cpu_instructions, 99);
  assert.equal(result.timestamp, 1_700_000_000_000);
});

test('simulationRecordToResult: keeps stored errors', () => {
  const { result } = simulationRecordToResult({ id: '8', error: 'trapped' }, '8');
  assert.equal(result.success, false);
  assert.equal(result.error, 'trapped');
  assert.equal(result.analysisReport, undefined);
});

test('simulationRecordToResult: rejects malformed responses', () => {
  assert.throws(() => simulationRecordToResult(null, '1'));
  assert.throws(() => simulationRecordToResult('nope', '1'));
});
