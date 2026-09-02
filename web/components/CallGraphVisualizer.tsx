'use client';

import React, { useEffect, useRef } from 'react';
import mermaid from 'mermaid';
import { sanitizeMermaidDefinition } from '../lib/security';

interface CallGraphVisualizerProps {
  /** Structured call graph from the `/analyze` response. */
  callGraph?: CallGraphInput | null;
  /**
   * Mermaid definition kept for backward compatibility. It is only used to
   * decide whether to show the empty state when no structured graph is
   * supplied; the graph itself is rendered from `callGraph`.
   */
  mermaidDefinition?: string;
}

/** Human-readable description of each gas band, used in the legend. */
const LEGEND = [
  { band: 'low' as const, label: `< ${formatGas(GAS_THRESHOLDS.LOW)}` },
  { band: 'medium' as const, label: `< ${formatGas(GAS_THRESHOLDS.HIGH)}` },
  { band: 'high' as const, label: `>= ${formatGas(GAS_THRESHOLDS.HIGH)}` },
];

  useEffect(() => {
    mermaid.initialize({
      startOnLoad: false,
      theme: 'dark',
      // `strict` sanitizes any HTML in labels/definitions instead of trusting
      // it, closing the XSS sink that `loose` + `htmlLabels` previously opened.
      securityLevel: 'strict',
      flowchart: {
        useMaxWidth: true,
        htmlLabels: false,
        curve: 'basis',
      },
    });
  }, []);

  useEffect(() => {
    let cancelled = false;

    const renderMermaid = async () => {
      const container = containerRef.current;
      if (!container || !mermaidDefinition) return;

      try {
        const sanitized = sanitizeMermaidDefinition(mermaidDefinition);
        if (!sanitized) return;

        const { svg } = await mermaid.render('mermaid-graph-' + Date.now(), sanitized);
        if (cancelled) return;
        container.innerHTML = '';
        container.innerHTML = svg;
      } catch (error) {
        console.error('Mermaid rendering failed:', error);
        if (cancelled || !containerRef.current) return;

        // Build the error node with the DOM APIs so the error text can never
        // be interpreted as HTML.
        containerRef.current.innerHTML = '';
        const message = document.createElement('p');
        message.style.color = '#fb8500';
        const label = document.createElement('span');
        label.textContent = 'Failed to render call graph: ';
        const detail = document.createElement('span');
        detail.textContent = error instanceof Error ? error.message : String(error);
        message.appendChild(label);
        message.appendChild(detail);
        containerRef.current.appendChild(message);
      }
    };

    renderMermaid();

    return () => {
      cancelled = true;
    };
  }, [mermaidDefinition]);

  return (
    <div
      className="rounded-md border-2 bg-[var(--bg-card)] px-3 py-2 text-left shadow-sm transition-shadow"
      style={{
        borderColor: selected ? color : 'var(--border-default)',
        boxShadow: selected ? `0 0 0 2px ${color}55` : undefined,
        minWidth: 170,
      }}
    >
      {!data.isRoot && <Handle type="target" position={Position.Left} />}

      <div className="flex items-center gap-1.5">
        <span
          className="inline-block h-2 w-2 shrink-0 rounded-full"
          style={{ backgroundColor: color }}
          aria-hidden="true"
        />
        <span className="font-mono text-[11px] text-[var(--text-secondary)]">
          {data.label}
        </span>
      </div>

      <div className="mt-0.5 font-mono text-sm font-semibold text-[var(--text-primary)]">
        {data.functionName || '(unnamed)'}
      </div>

      <div className="mt-0.5 font-mono text-[10px]" style={{ color }}>
        {data.gas === null ? 'gas n/a' : `${formatGas(data.gas)} gas`}
      </div>

      <Handle type="source" position={Position.Right} />
    </div>
  );
}

const NODE_TYPES = { callNode: CallNode };

export function CallGraphVisualizer({
  callGraph,
  mermaidDefinition,
}: CallGraphVisualizerProps) {
  const { nodes, edges } = useMemo(() => buildFlowElements(callGraph), [callGraph]);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // Drop a stale selection when a new simulation replaces the graph.
  useEffect(() => {
    setSelectedId(null);
  }, [callGraph]);

  const handleNodeClick = useCallback<NodeMouseHandler>((_event, node) => {
    setSelectedId((current) => (current === node.id ? null : node.id));
  }, []);

  const selectedStack = useMemo(
    () => (selectedId ? callStackFor(nodes, edges, selectedId) : []),
    [nodes, edges, selectedId],
  );

  const selected = selectedStack.length > 0 ? selectedStack[selectedStack.length - 1] : null;
  const aggregateGas = useMemo(() => totalGas(nodes), [nodes]);

  const flowNodes = useMemo(
    () => nodes.map((node) => ({ ...node, selected: node.id === selectedId })),
    [nodes, selectedId],
  );

  if (nodes.length === 0) {
    return (
      <div className="mt-5">
        <h4 className="mb-3 text-sm font-semibold text-[var(--text-primary)]">
          Cross-Contract Call Graph
        </h4>
        <div className="rounded-lg border border-[var(--border-default)] bg-[var(--bg-elevated)] p-6 text-center text-sm text-[var(--text-secondary)]">
          {mermaidDefinition
            ? 'This result predates structured call graphs, so it cannot be rendered interactively.'
            : 'This invocation made no cross-contract calls.'}
        </div>
      </div>
    );
  }

  return (
    <div className="mt-5">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h4 className="text-sm font-semibold text-[var(--text-primary)]">
          Cross-Contract Call Graph
        </h4>
        <span className="font-mono text-xs text-[var(--text-secondary)]">
          {nodes.length} calls
          {aggregateGas > 0 ? ` · ${formatGas(aggregateGas)} gas total` : ''}
        </span>
      </div>

      {/* Gas legend */}
      <div className="mb-2 flex flex-wrap items-center gap-3 text-[10px] text-[var(--text-secondary)]">
        {LEGEND.map((entry) => (
          <span key={entry.band} className="inline-flex items-center gap-1.5">
            <span
              className="inline-block h-0.5 w-4 rounded"
              style={{ backgroundColor: GAS_COLORS[entry.band] }}
              aria-hidden="true"
            />
            {entry.label} gas
          </span>
        ))}
      </div>

      <div
        className="overflow-hidden rounded-lg border border-[var(--border-default)] bg-[var(--bg-elevated)]"
        style={{ height: 380 }}
      >
        <ReactFlow
          nodes={flowNodes as Node<CallNodeData>[]}
          edges={edges as unknown as Edge[]}
          nodeTypes={NODE_TYPES}
          onNodeClick={handleNodeClick}
          onPaneClick={() => setSelectedId(null)}
          fitView
          fitViewOptions={{ padding: 0.2 }}
          minZoom={0.2}
          maxZoom={1.75}
          proOptions={{ hideAttribution: true }}
        >
          <Background gap={16} color="var(--border-subtle)" />
          <Controls showInteractive={false} />
          <MiniMap
            pannable
            zoomable
            nodeColor={(node) => GAS_COLORS[(node.data as CallNodeData).band]}
            style={{ backgroundColor: 'var(--bg-card)' }}
          />
        </ReactFlow>
      </div>

      {/* Call stack inspector */}
      {selected ? (
        <div className="mt-3 rounded-lg border border-[var(--border-default)] bg-[var(--bg-card)] p-3">
          <div className="mb-2 flex items-center justify-between gap-2">
            <h5 className="text-xs font-semibold uppercase tracking-wide text-[var(--text-primary)]">
              Call Stack
            </h5>
            <button
              type="button"
              onClick={() => setSelectedId(null)}
              className="text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
            >
              Clear
            </button>
          </div>

          <ol className="m-0 list-none p-0 font-mono text-xs">
            {selectedStack.map((node, index) => (
              <li
                key={node.id}
                className="flex items-baseline gap-2 py-0.5 text-[var(--text-secondary)]"
                style={{ paddingLeft: index * 12 }}
              >
                <span aria-hidden="true">{index === 0 ? '>' : '└'}</span>
                <span className="text-[var(--text-primary)]">
                  {node.data.functionName || '(unnamed)'}
                </span>
                <span className="text-[10px]">{node.data.label}</span>
                <span
                  className="ml-auto text-[10px]"
                  style={{ color: GAS_COLORS[node.data.band] }}
                >
                  {node.data.gas === null ? 'n/a' : formatGas(node.data.gas)}
                </span>
              </li>
            ))}
          </ol>

          <dl className="mt-3 grid grid-cols-1 gap-x-4 gap-y-1 border-t border-[var(--border-default)] pt-2 text-xs sm:grid-cols-2">
            <div className="flex justify-between gap-2">
              <dt className="text-[var(--text-secondary)]">Contract</dt>
              <dd
                className="truncate font-mono text-[var(--text-primary)]"
                title={selected.data.contractId}
              >
                {selected.data.label}
              </dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt className="text-[var(--text-secondary)]">Depth</dt>
              <dd className="font-mono text-[var(--text-primary)]">{selected.data.depth}</dd>
            </div>
          </dl>

          <div className="mt-2">
            <span className="text-xs text-[var(--text-secondary)]">Parameters</span>
            {selected.data.args.length > 0 ? (
              <pre className="mt-1 max-h-40 overflow-auto rounded border border-[var(--border-default)] bg-[var(--bg-elevated)] p-2 font-mono text-[11px] text-[var(--text-primary)]">
                {JSON.stringify(selected.data.args, null, 2)}
              </pre>
            ) : (
              <p className="mt-1 text-[11px] italic text-[var(--text-secondary)]">
                No decoded parameters available for this call.
              </p>
            )}
          </div>

          {selected.data.returnValue !== null && (
            <div className="mt-2">
              <span className="text-xs text-[var(--text-secondary)]">Returns</span>
              <pre className="mt-1 max-h-32 overflow-auto rounded border border-[var(--border-default)] bg-[var(--bg-elevated)] p-2 font-mono text-[11px] text-[var(--text-primary)]">
                {JSON.stringify(selected.data.returnValue, null, 2)}
              </pre>
            </div>
          )}
        </div>
      ) : (
        <p className="mt-2 text-xs text-[var(--text-secondary)]">
          Select a node to inspect its call stack and parameters.
        </p>
      )}
    </div>
  );
}