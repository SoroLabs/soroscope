// schemaGraph.test.cjs — unit tests for the React Flow schema graph model
// Closes Issue #628
// Runs with: node --test ./lib/schemaGraph.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  NODE_KIND,
  COLUMN_WIDTH,
  truncateContractId,
  flattenCallGraph,
  buildSchemaGraph,
  hasSchemaGraphData,
} = require('./schemaGraph');

const ROOT_CONTRACT = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWXYZ';
const TOKEN_CONTRACT = 'CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBTOKN';

function makeReport(overrides = {}) {
  return {
    call_graph: {
      root: {
        contract_id: ROOT_CONTRACT,
        function: 'swap',
        children: [
          { contract_id: TOKEN_CONTRACT, function: 'transfer', children: [] },
          { contract_id: TOKEN_CONTRACT, function: 'balance', children: [] },
        ],
      },
    },
    state_dependency: [{ key: 'Balance(GABC)', source: 'Live' }],
    state_snapshot: null,
    ttl_analysis: null,
    ...overrides,
  };
}

// ── Formatting ──────────────────────────────────────────────────────────────

test('truncateContractId: shortens long ids and leaves short ones alone', () => {
  assert.equal(truncateContractId(ROOT_CONTRACT), 'CAAA…WXYZ');
  assert.equal(truncateContractId('CABC'), 'CABC');
  assert.equal(truncateContractId(''), '');
});

// ── Call graph flattening ───────────────────────────────────────────────────

test('flattenCallGraph: returns an empty list for a missing root', () => {
  assert.deepEqual(flattenCallGraph(null), []);
  assert.deepEqual(flattenCallGraph(undefined), []);
});

test('flattenCallGraph: records depth and parent for each call', () => {
  const flattened = flattenCallGraph(makeReport().call_graph.root);
  assert.equal(flattened.length, 3);
  assert.equal(flattened[0].depth, 0);
  assert.equal(flattened[0].parentId, null);
  assert.equal(flattened[1].depth, 1);
  assert.equal(flattened[1].parentId, flattened[0].id);
  assert.equal(flattened[2].functionName, 'balance');
});

test('flattenCallGraph: breaks re-entrant cycles instead of looping forever', () => {
  const root = { contract_id: 'A', function: 'f', children: [] };
  const child = { contract_id: 'B', function: 'g', children: [root] };
  root.children.push(child);

  const flattened = flattenCallGraph(root);
  assert.equal(flattened.length, 3);
  assert.equal(flattened[2].isCycle, true);
});

// ── Graph building ──────────────────────────────────────────────────────────

test('buildSchemaGraph: returns an empty graph for a null report', () => {
  const graph = buildSchemaGraph(null);
  assert.deepEqual(graph.nodes, []);
  assert.deepEqual(graph.edges, []);
  assert.equal(graph.stats.contractNodes, 0);
  assert.equal(hasSchemaGraphData(null), false);
});

test('buildSchemaGraph: emits one node per unique call and an edge per parent link', () => {
  const graph = buildSchemaGraph(makeReport(), { includeStorage: false });
  assert.equal(graph.nodes.length, 3);
  assert.equal(graph.edges.length, 2);
  assert.equal(graph.stats.contractNodes, 3);
  assert.equal(graph.stats.maxDepth, 1);
  assert.equal(graph.nodes[0].type, NODE_KIND.CONTRACT);
  assert.equal(graph.nodes[0].data.isRoot, true);
  assert.equal(graph.nodes[1].data.isRoot, false);
});

test('buildSchemaGraph: lays children out in later columns and stacked rows', () => {
  const graph = buildSchemaGraph(makeReport(), { includeStorage: false });
  assert.deepEqual(graph.nodes[0].position, { x: 0, y: 0 });
  assert.equal(graph.nodes[1].position.x, COLUMN_WIDTH);
  assert.equal(graph.nodes[2].position.x, COLUMN_WIDTH);
  assert.notEqual(graph.nodes[1].position.y, graph.nodes[2].position.y);
});

test('buildSchemaGraph: deduplicates repeated calls to the same contract function', () => {
  const report = makeReport({
    call_graph: {
      root: {
        contract_id: ROOT_CONTRACT,
        function: 'swap',
        children: [
          { contract_id: TOKEN_CONTRACT, function: 'transfer', children: [] },
          { contract_id: TOKEN_CONTRACT, function: 'transfer', children: [] },
        ],
      },
    },
  });

  const graph = buildSchemaGraph(report, { includeStorage: false });
  assert.equal(graph.nodes.length, 2);
  assert.equal(graph.edges.length, 1);
});

test('buildSchemaGraph: gives every node and edge a unique id', () => {
  const graph = buildSchemaGraph(makeReport());
  const nodeIds = graph.nodes.map((n) => n.id);
  const edgeIds = graph.edges.map((e) => e.id);
  assert.equal(new Set(nodeIds).size, nodeIds.length);
  assert.equal(new Set(edgeIds).size, edgeIds.length);
});

test('buildSchemaGraph: every edge references existing nodes', () => {
  const graph = buildSchemaGraph(makeReport());
  const ids = new Set(graph.nodes.map((n) => n.id));
  graph.edges.forEach((edge) => {
    assert.ok(ids.has(edge.source), `missing source ${edge.source}`);
    assert.ok(ids.has(edge.target), `missing target ${edge.target}`);
  });
});

test('buildSchemaGraph: flags re-entrant edges', () => {
  const root = { contract_id: 'A', function: 'f', children: [] };
  root.children.push({ contract_id: 'B', function: 'g', children: [root] });

  const graph = buildSchemaGraph({ call_graph: { root } }, { includeStorage: false });
  assert.equal(graph.stats.hasCycle, true);
  const cycleEdge = graph.edges.find((edge) => edge.data.isCycle);
  assert.equal(cycleEdge.label, 're-entrant');
});

// ── Storage mapping ─────────────────────────────────────────────────────────

test('buildSchemaGraph: renders state dependencies as read storage nodes', () => {
  const graph = buildSchemaGraph(makeReport());
  const storage = graph.nodes.filter((n) => n.type === NODE_KIND.STORAGE);
  assert.equal(storage.length, 1);
  assert.equal(storage[0].data.storageKey, 'Balance(GABC)');
  assert.equal(storage[0].data.written, false);

  const storageEdge = graph.edges.find((e) => e.data.kind === 'storage');
  assert.equal(storageEdge.label, 'reads');
  assert.equal(storageEdge.source, graph.nodes[0].id);
});

test('buildSchemaGraph: marks snapshot ledger entries as written', () => {
  const graph = buildSchemaGraph(
    makeReport({
      state_snapshot: {
        ledger_entries: { 'Balance(GABC)': '0xdead' },
        ttl_entries: {},
        latest_ledger: 42,
      },
    }),
  );

  const storage = graph.nodes.filter((n) => n.type === NODE_KIND.STORAGE);
  assert.equal(storage.length, 1);
  assert.equal(storage[0].data.written, true);
  assert.equal(graph.edges.find((e) => e.data.kind === 'storage').label, 'writes');
});

test('buildSchemaGraph: attaches TTL remaining ledgers to storage nodes', () => {
  const graph = buildSchemaGraph(
    makeReport({
      ttl_analysis: {
        current_ledger: 100,
        touched_entries: [
          { key: 'Balance(GABC)', live_until_ledger: 500, remaining_ledgers: 400 },
        ],
        extend_ttl_suggestions: [],
      },
    }),
  );

  const storage = graph.nodes.find((n) => n.type === NODE_KIND.STORAGE);
  assert.equal(storage.data.remainingLedgers, 400);
});

test('buildSchemaGraph: caps storage nodes and reports the hidden count', () => {
  const state_dependency = Array.from({ length: 20 }, (_, i) => ({
    key: `Key(${i})`,
    source: 'Live',
  }));

  const graph = buildSchemaGraph(makeReport({ state_dependency }), { maxStorageNodes: 5 });
  assert.equal(graph.stats.storageNodes, 5);
  assert.equal(graph.stats.hiddenStorageNodes, 15);
});

test('buildSchemaGraph: includeStorage:false drops storage nodes entirely', () => {
  const graph = buildSchemaGraph(makeReport(), { includeStorage: false });
  assert.equal(graph.nodes.filter((n) => n.type === NODE_KIND.STORAGE).length, 0);
  assert.equal(graph.stats.storageNodes, 0);
});

test('hasSchemaGraphData: true once a call graph exists', () => {
  assert.equal(hasSchemaGraphData(makeReport()), true);
  assert.equal(hasSchemaGraphData({ call_graph: null }), false);
});
