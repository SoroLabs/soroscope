/**
 * Builds the React Flow node/edge model for the subgraph schema visualizer.
 *
 * The layout is deliberately deterministic (no physics simulation) so the same
 * analysis report always renders the same diagram and the whole thing stays
 * unit-testable without a DOM.
 */

const NODE_KIND = {
  CONTRACT: 'contract',
  STORAGE: 'storage',
};

const COLUMN_WIDTH = 280;
const ROW_HEIGHT = 110;
const STORAGE_COLUMN_GAP = 120;

/** Shorten a Soroban contract id for display: `CAAA…WXYZ`. */
function truncateContractId(contractId, lead = 4, tail = 4) {
  const value = String(contractId || '');
  if (value.length <= lead + tail + 1) return value;
  return `${value.slice(0, lead)}…${value.slice(-tail)}`;
}

function nodeIdFor(contractId, functionName) {
  return `fn:${contractId}::${functionName}`;
}

function storageNodeIdFor(key) {
  return `store:${key}`;
}

/**
 * Flatten the call tree breadth-first, recording each node's depth and its
 * parent so edges can be emitted in one pass.
 *
 * Cycles (A → B → A) are broken by tracking the ancestor chain: the repeated
 * call still produces an edge, but is not expanded again.
 */
function flattenCallGraph(root) {
  if (!root || typeof root !== 'object') return [];

  const flattened = [];
  const queue = [{ node: root, depth: 0, parentId: null, ancestors: [] }];

  while (queue.length > 0) {
    const { node, depth, parentId, ancestors } = queue.shift();
    const contractId = String(node.contract_id || 'unknown');
    const functionName = String(node.function || 'unknown');
    const id = nodeIdFor(contractId, functionName);
    const isCycle = ancestors.includes(id);

    flattened.push({ id, contractId, functionName, depth, parentId, isCycle });

    if (isCycle) continue;

    const children = Array.isArray(node.children) ? node.children : [];
    children.forEach((child) => {
      queue.push({ node: child, depth: depth + 1, parentId: id, ancestors: [...ancestors, id] });
    });
  }

  return flattened;
}

function collectStorageKeys(report) {
  const keys = new Map();

  const dependencies = Array.isArray(report && report.state_dependency)
    ? report.state_dependency
    : [];
  dependencies.forEach((entry) => {
    if (!entry || !entry.key) return;
    keys.set(entry.key, { key: entry.key, source: entry.source || 'Live', written: false });
  });

  const snapshot = report && report.state_snapshot;
  const ledgerEntries = snapshot && snapshot.ledger_entries ? snapshot.ledger_entries : {};
  Object.keys(ledgerEntries).forEach((key) => {
    const existing = keys.get(key);
    keys.set(key, {
      key,
      source: existing ? existing.source : 'Live',
      // A key present in the post-simulation snapshot was touched by the call.
      written: true,
    });
  });

  const ttlEntries = report && report.ttl_analysis ? report.ttl_analysis.touched_entries : null;
  if (Array.isArray(ttlEntries)) {
    ttlEntries.forEach((entry) => {
      if (!entry || !entry.key) return;
      const existing = keys.get(entry.key);
      keys.set(entry.key, {
        key: entry.key,
        source: existing ? existing.source : 'Live',
        written: existing ? existing.written : false,
        remainingLedgers: entry.remaining_ledgers,
      });
    });
  }

  return Array.from(keys.values());
}

/**
 * Convert an analysis report into `{ nodes, edges, stats }` consumable by
 * React Flow.
 *
 * @param {object|null} report `ResourceReport` from `/analyze`.
 * @param {{ includeStorage?: boolean, maxStorageNodes?: number }} [options]
 */
function buildSchemaGraph(report, options = {}) {
  const includeStorage = options.includeStorage !== false;
  const maxStorageNodes =
    typeof options.maxStorageNodes === 'number' ? options.maxStorageNodes : 12;

  const callGraph = report && report.call_graph ? report.call_graph : null;
  const flattened = flattenCallGraph(callGraph && callGraph.root);

  const nodes = [];
  const edges = [];
  const rowsPerDepth = new Map();
  const seen = new Set();

  flattened.forEach((entry) => {
    if (!seen.has(entry.id)) {
      seen.add(entry.id);
      const row = rowsPerDepth.get(entry.depth) || 0;
      rowsPerDepth.set(entry.depth, row + 1);

      nodes.push({
        id: entry.id,
        type: NODE_KIND.CONTRACT,
        position: { x: entry.depth * COLUMN_WIDTH, y: row * ROW_HEIGHT },
        data: {
          kind: NODE_KIND.CONTRACT,
          label: entry.functionName,
          contractId: entry.contractId,
          shortContractId: truncateContractId(entry.contractId),
          functionName: entry.functionName,
          depth: entry.depth,
          isRoot: entry.depth === 0,
        },
      });
    }

    if (entry.parentId) {
      const edgeId = `edge:${entry.parentId}->${entry.id}`;
      if (!edges.some((edge) => edge.id === edgeId)) {
        edges.push({
          id: edgeId,
          source: entry.parentId,
          target: entry.id,
          animated: !entry.isCycle,
          label: entry.isCycle ? 're-entrant' : 'calls',
          data: { kind: 'call', isCycle: entry.isCycle },
        });
      }
    }
  });

  const maxDepth = flattened.reduce((acc, entry) => Math.max(acc, entry.depth), 0);
  const rootId = flattened.length > 0 ? flattened[0].id : null;

  let storageKeys = [];
  if (includeStorage) {
    storageKeys = collectStorageKeys(report);
    const visible = storageKeys.slice(0, maxStorageNodes);

    visible.forEach((entry, index) => {
      const id = storageNodeIdFor(entry.key);
      nodes.push({
        id,
        type: NODE_KIND.STORAGE,
        position: {
          x: (maxDepth + 1) * COLUMN_WIDTH + STORAGE_COLUMN_GAP,
          y: index * (ROW_HEIGHT - 20),
        },
        data: {
          kind: NODE_KIND.STORAGE,
          label: entry.key,
          storageKey: entry.key,
          source: entry.source,
          written: entry.written,
          remainingLedgers: entry.remainingLedgers ?? null,
        },
      });

      if (rootId) {
        edges.push({
          id: `edge:${rootId}->${id}`,
          source: rootId,
          target: id,
          animated: false,
          label: entry.written ? 'writes' : 'reads',
          data: { kind: 'storage', written: entry.written },
        });
      }
    });
  }

  return {
    nodes,
    edges,
    stats: {
      contractNodes: seen.size,
      storageNodes: nodes.filter((node) => node.type === NODE_KIND.STORAGE).length,
      hiddenStorageNodes: Math.max(0, storageKeys.length - maxStorageNodes),
      maxDepth,
      hasCycle: flattened.some((entry) => entry.isCycle),
    },
  };
}

/** True when a report carries enough data to render a meaningful diagram. */
function hasSchemaGraphData(report) {
  const graph = buildSchemaGraph(report);
  return graph.nodes.length > 0;
}

module.exports = {
  NODE_KIND,
  COLUMN_WIDTH,
  ROW_HEIGHT,
  truncateContractId,
  flattenCallGraph,
  buildSchemaGraph,
  hasSchemaGraphData,
};
