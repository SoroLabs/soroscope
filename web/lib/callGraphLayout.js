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
 * Assign positions using an improved layered layout that prevents the edge
 * overlap and centre-clustering problems seen with >20 nodes (issue #1299).
 *
 * The algorithm:
 * 1. Build a parent → children map so we know the subtree structure.
 * 2. Walk the tree post-order to compute each node's "subtree height" — the
 *    vertical space its descendants occupy.
 * 3. Assign Y positions top-down: each child is placed relative to its
 *    parent's centre, offset so siblings stack without overlap.
 * 4. Levels are centred vertically against the tallest column.
 *
 * The result is deterministic (re-renders don't shuffle nodes) and keeps
 * edges from crossing through unrelated subtrees.
 */
function layoutNodes(flat) {
  if (flat.length === 0) return [];

  // ── Tuning constants ───────────────────────────────────────────────────
  // For large graphs (>20 nodes) we increase spacing so nodes don't crowd.
  const isLarge = flat.length > 20;
  const levelWidth = isLarge ? Math.round(LEVEL_WIDTH * 1.15) : LEVEL_WIDTH;
  const rowHeight = isLarge ? Math.round(ROW_HEIGHT * 1.2) : ROW_HEIGHT;
  // Vertical gap between sibling subtrees.
  const siblingGap = isLarge ? 20 : 8;

  // ── Build adjacency maps ──────────────────────────────────────────────
  const childrenOf = new Map(); // parentId -> array of flat entries
  const byId = new Map();       // id -> flat entry

  for (const entry of flat) {
    byId.set(entry.id, entry);
    const parentKey = entry.parentId ?? '__root__';
    if (!childrenOf.has(parentKey)) childrenOf.set(parentKey, []);
    childrenOf.get(parentKey).push(entry);
  }

  // ── Post-order: compute subtree height for each node ───────────────────
  // subtreeHeight = max(rowHeight, sum of children heights + gaps)
  const subtreeHeights = new Map();

  function computeSubtreeHeight(id) {
    if (subtreeHeights.has(id)) return subtreeHeights.get(id);
    const children = childrenOf.get(id) || [];
    let h;
    if (children.length === 0) {
      h = rowHeight;
    } else {
      h = children.reduce((sum, child, i) => {
        const childH = computeSubtreeHeight(child.id);
        return sum + childH + (i > 0 ? siblingGap : 0);
      }, 0);
      // Ensure a parent is at least as tall as a single row.
      h = Math.max(h, rowHeight);
    }
    subtreeHeights.set(id, h);
    return h;
  }

  // Compute heights starting from root(s).
  const roots = childrenOf.get('__root__') || [];
  for (const root of roots) computeSubtreeHeight(root.id);

  // ── Top-down: assign Y positions ───────────────────────────────────────
  // Each parent is centred vertically within its subtree. Children are laid
  // out top-to-bottom within the parent's allocated space.
  const positions = new Map();

  function assignPositions(id, topY) {
    const height = subtreeHeights.get(id) || rowHeight;
    const entry = byId.get(id);
    const children = childrenOf.get(id) || [];

    // Centre this node within its subtree height.
    const centerY = topY + height / 2 - rowHeight / 2;

    positions.set(id, { y: centerY });

    // Lay out children top-to-bottom, each within its own subtree height.
    let childY = topY;
    for (let i = 0; i < children.length; i++) {
      const child = children[i];
      const childH = subtreeHeights.get(child.id) || rowHeight;
      assignPositions(child.id, childY);
      childY += childH + siblingGap;
    }
  }

  for (const root of roots) assignPositions(root.id, 0);

  // ── Centre each depth level vertically ─────────────────────────────────
  // Group by depth, find the max Y range per level, then shift so each level
  // is centred against the tallest level.
  const byDepth = new Map();
  let maxDepth = 0;
  for (const entry of flat) {
    if (!byDepth.has(entry.depth)) byDepth.set(entry.depth, []);
    byDepth.get(entry.depth).push(entry);
    if (entry.depth > maxDepth) maxDepth = entry.depth;
  }

  // Compute the vertical span of each depth level.
  const depthSpans = new Map();
  for (const [depth, entries] of byDepth) {
    let minTop = Infinity;
    let maxBottom = -Infinity;
    for (const entry of entries) {
      const y = positions.get(entry.id)?.y ?? 0;
      minTop = Math.min(minTop, y);
      maxBottom = Math.max(maxBottom, y + rowHeight);
    }
    depthSpans.set(depth, { minTop, maxBottom, height: maxBottom - minTop });
  }

  // Find the tallest level so we can centre everything against it.
  let maxLevelHeight = 0;
  for (const span of depthSpans.values()) {
    if (span.height > maxLevelHeight) maxLevelHeight = span.height;
  }

  // Shift each level so it is centred.
  const depthOffsets = new Map();
  for (const [depth, span] of depthSpans) {
    depthOffsets.set(depth, (maxLevelHeight - span.height) / 2 - span.minTop);
  }

  // ── Build the final positioned array ───────────────────────────────────
  return flat.map((entry) => {
    const pos = positions.get(entry.id) || { y: 0 };
    const offset = depthOffsets.get(entry.depth) || 0;
    return {
      ...entry,
      position: {
        x: entry.depth * levelWidth,
        y: pos.y + offset,
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
  const flat = flattenGraph(graph);
  const positioned = layoutNodes(flat);

  // Use smoothstep edges for large graphs (>20 nodes) so edges curve around
  // intermediate nodes rather than drawing straight lines that overlap (issue #1299).
  const edgeType = flat.length > 20 ? 'smoothstep' : 'default';

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
        type: edgeType,
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
