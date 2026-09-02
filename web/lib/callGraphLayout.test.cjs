// callGraphLayout.test.cjs — unit tests for the call graph layout helpers
// Closes Issue #41
// Runs with: node --test ./lib/callGraphLayout.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  GAS_THRESHOLDS,
  GAS_COLORS,
  gasBand,
  gasColor,
  truncateContractId,
  flattenGraph,
  buildFlowElements,
  formatGas,
  callStackFor,
  totalGas,
} = require('./callGraphLayout.js');

// ── gasBand / gasColor ───────────────────────────────────────────────────────

test('gasBand applies the thresholds from the issue', () => {
  assert.equal(gasBand(0), 'low');
  assert.equal(gasBand(99_999), 'low');
  assert.equal(gasBand(GAS_THRESHOLDS.LOW), 'medium');
  assert.equal(gasBand(499_999), 'medium');
  assert.equal(gasBand(GAS_THRESHOLDS.HIGH), 'high');
  assert.equal(gasBand(2_000_000), 'high');
});

test('gasBand reports unknown rather than cheap when there is no reading', () => {
  assert.equal(gasBand(null), 'unknown');
  assert.equal(gasBand(undefined), 'unknown');
  assert.equal(gasBand(NaN), 'unknown');
  assert.equal(gasBand(-1), 'unknown');
});

test('gasBand accepts numeric strings', () => {
  assert.equal(gasBand('50000'), 'low');
  assert.equal(gasBand('600000'), 'high');
});

test('gasColor maps each band to its colour', () => {
  assert.equal(gasColor(1_000), GAS_COLORS.low);
  assert.equal(gasColor(200_000), GAS_COLORS.medium);
  assert.equal(gasColor(900_000), GAS_COLORS.high);
  assert.equal(gasColor(null), GAS_COLORS.unknown);
});

// ── truncateContractId ───────────────────────────────────────────────────────

test('truncateContractId shortens long addresses and leaves short ones alone', () => {
  assert.equal(truncateContractId('CABCDEFGHIJKLMNOPQRSTUVWXYZ'), 'CABCDEFG...WXYZ');
  assert.equal(truncateContractId('Host'), 'Host');
  assert.equal(truncateContractId(''), '');
  assert.equal(truncateContractId(undefined), '');
});

// ── flattenGraph ─────────────────────────────────────────────────────────────

const GRAPH = {
  root: {
    contract_id: 'CROOT00000000000000000000000000000000000000000000000AAAA',
    function: 'swap',
    gas_used: 50_000,
    children: [
      {
        contract_id: 'CPOOL00000000000000000000000000000000000000000000000BBBB',
        function: 'transfer',
        gas_used: 250_000,
        children: [],
      },
      {
        contract_id: 'CORCL00000000000000000000000000000000000000000000000CCCC',
        function: 'get_price',
        gas_used: 900_000,
        children: [
          {
            contract_id: 'CFEED00000000000000000000000000000000000000000000000DDDD',
            function: 'read',
            gas_used: 10_000,
            children: [],
          },
        ],
      },
    ],
  },
};

test('flattenGraph visits every node once and records depth', () => {
  const flat = flattenGraph(GRAPH);
  assert.equal(flat.length, 4);
  assert.deepEqual(
    flat.map((n) => n.functionName),
    ['swap', 'transfer', 'get_price', 'read'],
  );
  assert.deepEqual(
    flat.map((n) => n.depth),
    [0, 1, 1, 2],
  );
});

test('flattenGraph links each node to its parent, with the root unparented', () => {
  const flat = flattenGraph(GRAPH);
  assert.equal(flat[0].parentId, null);
  assert.equal(flat[1].parentId, flat[0].id);
  assert.equal(flat[2].parentId, flat[0].id);
  assert.equal(flat[3].parentId, flat[2].id);
});

test('flattenGraph returns an empty list for missing or malformed input', () => {
  assert.deepEqual(flattenGraph(null), []);
  assert.deepEqual(flattenGraph(undefined), []);
  assert.deepEqual(flattenGraph({}), []);
  assert.deepEqual(flattenGraph({ root: null }), []);
});

test('flattenGraph terminates on a cyclic graph instead of hanging', () => {
  const cyclic = { contract_id: 'CA', function: 'loop', children: [] };
  cyclic.children.push(cyclic);
  const flat = flattenGraph({ root: cyclic });
  assert.equal(flat.length, 1);
});

test('flattenGraph tolerates missing children and gas fields', () => {
  const flat = flattenGraph({ root: { contract_id: 'CA', function: 'noop' } });
  assert.equal(flat.length, 1);
  assert.equal(flat[0].gas, null);
  assert.deepEqual(flat[0].args, []);
});

// ── buildFlowElements ────────────────────────────────────────────────────────

test('buildFlowElements produces one node per call and one edge per parent link', () => {
  const { nodes, edges } = buildFlowElements(GRAPH);
  assert.equal(nodes.length, 4);
  assert.equal(edges.length, 3);
});

test('buildFlowElements lays nodes out in columns by depth', () => {
  const { nodes } = buildFlowElements(GRAPH);
  assert.equal(nodes[0].position.x, 0);
  assert.equal(nodes[1].position.x, nodes[2].position.x);
  assert.ok(nodes[3].position.x > nodes[1].position.x);
  // Siblings at the same depth must not overlap.
  assert.notEqual(nodes[1].position.y, nodes[2].position.y);
});

test('buildFlowElements colours edges by the callee gas band', () => {
  const { nodes, edges } = buildFlowElements(GRAPH);
  const byTargetFunction = (fn) => {
    const node = nodes.find((n) => n.data.functionName === fn);
    return edges.find((e) => e.target === node.id);
  };

  assert.equal(byTargetFunction('transfer').style.stroke, GAS_COLORS.medium);
  assert.equal(byTargetFunction('get_price').style.stroke, GAS_COLORS.high);
  assert.equal(byTargetFunction('read').style.stroke, GAS_COLORS.low);
});

test('buildFlowElements draws costly edges heavier and animates them', () => {
  const { nodes, edges } = buildFlowElements(GRAPH);
  const oracle = nodes.find((n) => n.data.functionName === 'get_price');
  const edge = edges.find((e) => e.target === oracle.id);
  assert.equal(edge.animated, true);
  assert.equal(edge.style.strokeWidth, 3);
});

test('buildFlowElements marks the root node and truncates contract labels', () => {
  const { nodes } = buildFlowElements(GRAPH);
  assert.equal(nodes[0].data.isRoot, true);
  assert.equal(nodes[1].data.isRoot, false);
  assert.ok(nodes[0].data.label.includes('...'));
});

test('buildFlowElements leaves gas-less edges unlabelled and neutral', () => {
  const { edges } = buildFlowElements({
    root: {
      contract_id: 'CA',
      function: 'root',
      children: [{ contract_id: 'CB', function: 'child' }],
    },
  });
  assert.equal(edges.length, 1);
  assert.equal(edges[0].label, undefined);
  assert.equal(edges[0].style.stroke, GAS_COLORS.unknown);
});

test('buildFlowElements returns empty arrays for an absent graph', () => {
  assert.deepEqual(buildFlowElements(null), { nodes: [], edges: [] });
  assert.deepEqual(buildFlowElements(undefined), { nodes: [], edges: [] });
});

test('buildFlowElements gives every node and edge a unique id', () => {
  const { nodes, edges } = buildFlowElements(GRAPH);
  assert.equal(new Set(nodes.map((n) => n.id)).size, nodes.length);
  assert.equal(new Set(edges.map((e) => e.id)).size, edges.length);
});

// ── formatGas ────────────────────────────────────────────────────────────────

test('formatGas renders compact figures and flags missing readings', () => {
  assert.equal(formatGas(950), '950');
  assert.equal(formatGas(250_000), '250K');
  assert.equal(formatGas(1_200_000), '1.2M');
  assert.equal(formatGas(null), 'n/a');
  assert.equal(formatGas(NaN), 'n/a');
});

// ── callStackFor ─────────────────────────────────────────────────────────────

test('callStackFor returns the root-to-node path in call order', () => {
  const { nodes, edges } = buildFlowElements(GRAPH);
  const leaf = nodes.find((n) => n.data.functionName === 'read');
  const stack = callStackFor(nodes, edges, leaf.id);

  assert.deepEqual(
    stack.map((n) => n.data.functionName),
    ['swap', 'get_price', 'read'],
  );
});

test('callStackFor returns just the root when the root is selected', () => {
  const { nodes, edges } = buildFlowElements(GRAPH);
  const stack = callStackFor(nodes, edges, nodes[0].id);
  assert.equal(stack.length, 1);
  assert.equal(stack[0].data.functionName, 'swap');
});

test('callStackFor is empty for an unknown node id', () => {
  const { nodes, edges } = buildFlowElements(GRAPH);
  assert.deepEqual(callStackFor(nodes, edges, 'missing'), []);
  assert.deepEqual(callStackFor(nodes, edges, null), []);
});

// ── totalGas ─────────────────────────────────────────────────────────────────

test('totalGas sums every reported reading', () => {
  const { nodes } = buildFlowElements(GRAPH);
  assert.equal(totalGas(nodes), 50_000 + 250_000 + 900_000 + 10_000);
});

test('totalGas ignores nodes without a usable reading', () => {
  const { nodes } = buildFlowElements({
    root: {
      contract_id: 'CA',
      function: 'root',
      gas_used: 1_000,
      children: [{ contract_id: 'CB', function: 'child' }],
    },
  });
  assert.equal(totalGas(nodes), 1_000);
  assert.equal(totalGas([]), 0);
  assert.equal(totalGas(undefined), 0);
});
