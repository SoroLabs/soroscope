// Pure helpers that turn a Soroban call graph into React Flow nodes and edges.
//
// Keeping the traversal, layout and gas classification here (rather than inside
// the component) makes them directly unit-testable and keeps the component
// concerned only with rendering and interaction.

/**
 * Gas thresholds used to colour edges, per issue #41.
 *
 * An edge is green below LOW, yellow below HIGH, and red at or above HIGH.
 */
const GAS_THRESHOLDS = Object.freeze({
  LOW: 100_000,
  HIGH: 500_000,
});

/** Edge colours per gas band. */
const GAS_COLORS = Object.freeze({
  low: '#2da44e',
  medium: '#bf8700',
  high: '#cf222e',
  unknown: '#8b949e',
});

/** Horizontal spacing between depth levels, in pixels. */
const LEVEL_WIDTH = 260;
/** Vertical spacing between sibling nodes, in pixels. */
const ROW_HEIGHT = 90;

/**
 * Classify a gas reading into a colour band.
 *
 * Returns `'unknown'` when no usable reading is available, so the caller can
 * render the edge neutrally instead of implying it is cheap.
 *
 * @param {number|null|undefined} gas
 * @returns {'low'|'medium'|'high'|'unknown'}
 */
function gasBand(gas) {
  if (gas === null || gas === undefined) return 'unknown';
  const value = typeof gas === 'number' ? gas : Number(gas);
  if (!Number.isFinite(value) || value < 0) return 'unknown';
  if (value < GAS_THRESHOLDS.LOW) return 'low';
  if (value < GAS_THRESHOLDS.HIGH) return 'medium';
  return 'high';
}

/** Colour for a gas reading. */
function gasColor(gas) {
  return GAS_COLORS[gasBand(gas)];
}

/** Shorten a contract address for display: `CABC1234...WXYZ`. */
function truncateContractId(contractId) {
  const id = String(contractId || '');
  if (id.length <= 14) return id;
  return `${id.slice(0, 8)}...${id.slice(-4)}`;
}

/**
 * Flatten a call graph into a list of nodes carrying depth and parent links.
 *
 * The traversal is iterative and tracks visited node objects, so a malformed
 * graph containing a cycle cannot hang the UI.
 *
 * @param {{root: object}|null|undefined} graph
 */
function flattenGraph(graph) {
  const root = graph && graph.root;
  if (!root || typeof root !== 'object') return [];

  const flat = [];
  const seen = new Set();
  // Depth-first, pushing children in reverse so siblings keep source order.
  const stack = [{ node: root, depth: 0, parentId: null, index: 0 }];

  while (stack.length > 0) {
    const { node, depth, parentId } = stack.pop();
    if (!node || typeof node !== 'object' || seen.has(node)) continue;
    seen.add(node);

    const id = `${depth}-${flat.length}`;
    flat.push({
      id,
      depth,
      parentId,
      contractId: String(node.contract_id || ''),
      functionName: String(node.function || ''),
      // Optional enrichment; absent on graphs from older backends.
      gas: node.gas_used === undefined ? null : node.gas_used,
      args: Array.isArray(node.args) ? node.args : [],
      returnValue: node.return_value === undefined ? null : node.return_value,
    });

    const children = Array.isArray(node.children) ? node.children : [];
    for (let i = children.length - 1; i >= 0; i -= 1) {
      stack.push({ node: children[i], depth: depth + 1, parentId: id, index: i });
    }
  }

  return flat;
}

/**
 * Assign positions using a simple layered layout: depth drives the column,
 * order within a depth drives the row. Deterministic, so re-renders do not
 * shuffle the diagram.
 */
function layoutNodes(flat) {
  const rowsAtDepth = new Map();

  return flat.map((entry) => {
    const row = rowsAtDepth.get(entry.depth) || 0;
    rowsAtDepth.set(entry.depth, row + 1);

    return {
      ...entry,
      position: {
        x: entry.depth * LEVEL_WIDTH,
        y: row * ROW_HEIGHT,
      },
    };
  });
}

/**
 * Build the React Flow node and edge arrays for a call graph.
 *
 * Each edge inherits the gas reading of the call it represents (the child
 * node), which is what determines its colour and thickness.
 *
 * @param {{root: object}|null|undefined} graph
 * @returns {{nodes: Array, edges: Array}}
 */
function buildFlowElements(graph) {
  const positioned = layoutNodes(flattenGraph(graph));

  const nodes = positioned.map((entry) => ({
    id: entry.id,
    position: entry.position,
    type: 'callNode',
    data: {
      contractId: entry.contractId,
      label: truncateContractId(entry.contractId),
      functionName: entry.functionName,
      gas: entry.gas,
      band: gasBand(entry.gas),
      args: entry.args,
      returnValue: entry.returnValue,
      depth: entry.depth,
      isRoot: entry.depth === 0,
    },
  }));

  const edges = positioned
    .filter((entry) => entry.parentId !== null)
    .map((entry) => {
      const band = gasBand(entry.gas);
      const color = GAS_COLORS[band];

      return {
        id: `${entry.parentId}->${entry.id}`,
        source: entry.parentId,
        target: entry.id,
        animated: band === 'high',
        // A readable gas figure doubles as the edge label.
        label: entry.gas === null ? undefined : formatGas(entry.gas),
        style: {
          stroke: color,
          // Heavier strokes for costlier calls, so the expensive path reads
          // first even before the colour registers.
          strokeWidth: band === 'high' ? 3 : band === 'medium' ? 2 : 1.5,
        },
        labelStyle: { fill: color, fontSize: 10, fontWeight: 600 },
        markerEnd: { type: 'arrowclosed', color },
        data: { gas: entry.gas, band },
      };
    });

  return { nodes, edges };
}

/** Format a gas figure compactly, e.g. `250K`. */
function formatGas(gas) {
  // `Number(null)` is 0, so guard the absent cases explicitly -- rendering a
  // missing reading as "0" would imply the call was free.
  if (gas === null || gas === undefined || gas === '') return 'n/a';
  const value = typeof gas === 'number' ? gas : Number(gas);
  if (!Number.isFinite(value) || value < 0) return 'n/a';
  return new Intl.NumberFormat('en-US', {
    notation: 'compact',
    compactDisplay: 'short',
    maximumFractionDigits: 1,
  }).format(value);
}

/**
 * Walk from the root to the given node, producing the call stack that leads
 * to it. Used by the inspector panel when a node is selected.
 *
 * @param {Array} nodes React Flow nodes from `buildFlowElements`
 * @param {Array} edges React Flow edges from `buildFlowElements`
 * @param {string} nodeId
 */
function callStackFor(nodes, edges, nodeId) {
  const byId = new Map((nodes || []).map((n) => [n.id, n]));
  const parentOf = new Map((edges || []).map((e) => [e.target, e.source]));

  const stack = [];
  let current = nodeId;
  const guard = new Set();

  while (current && byId.has(current) && !guard.has(current)) {
    guard.add(current);
    stack.unshift(byId.get(current));
    current = parentOf.get(current);
  }

  return stack;
}

/** Aggregate gas across every call that reports it. */
function totalGas(nodes) {
  return (nodes || []).reduce((sum, node) => {
    const gas = node && node.data ? node.data.gas : null;
    const value = typeof gas === 'number' ? gas : Number(gas);
    return Number.isFinite(value) && value > 0 ? sum + value : sum;
  }, 0);
}

module.exports = {
  GAS_THRESHOLDS,
  GAS_COLORS,
  LEVEL_WIDTH,
  ROW_HEIGHT,
  gasBand,
  gasColor,
  truncateContractId,
  flattenGraph,
  layoutNodes,
  buildFlowElements,
  formatGas,
  callStackFor,
  totalGas,
};
