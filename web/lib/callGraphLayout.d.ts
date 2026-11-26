export type GasBand = 'low' | 'medium' | 'high' | 'unknown';

/** A node in the call graph returned by the backend. */
export interface CallGraphNode {
  contract_id: string;
  function: string;
  children?: CallGraphNode[];
  /** Gas attributed to this call. Absent on graphs from older backends. */
  gas_used?: number | null;
  /** Decoded call arguments, when the backend supplies them. */
  args?: unknown[];
  return_value?: unknown;
}

export interface CallGraphInput {
  root: CallGraphNode;
}

export interface FlatCallNode {
  id: string;
  depth: number;
  parentId: string | null;
  contractId: string;
  functionName: string;
  gas: number | null;
  args: unknown[];
  returnValue: unknown;
}

export interface PositionedCallNode extends FlatCallNode {
  position: { x: number; y: number };
}

export interface CallNodeData {
  contractId: string;
  /** Truncated contract address for display. */
  label: string;
  functionName: string;
  gas: number | null;
  band: GasBand;
  args: unknown[];
  returnValue: unknown;
  depth: number;
  isRoot: boolean;
}

export interface FlowNode {
  id: string;
  position: { x: number; y: number };
  type: string;
  data: CallNodeData;
}

export interface FlowEdge {
  id: string;
  source: string;
  target: string;
  animated: boolean;
  label?: string;
  style: { stroke: string; strokeWidth: number };
  labelStyle: { fill: string; fontSize: number; fontWeight: number };
  markerEnd: { type: string; color: string };
  data: { gas: number | null; band: GasBand };
}

export const GAS_THRESHOLDS: Readonly<{ LOW: number; HIGH: number }>;
export const GAS_COLORS: Readonly<Record<GasBand, string>>;
export const LEVEL_WIDTH: number;
export const ROW_HEIGHT: number;

export function gasBand(gas: number | null | undefined): GasBand;
export function gasColor(gas: number | null | undefined): string;
export function truncateContractId(contractId: string): string;
export function flattenGraph(graph: CallGraphInput | null | undefined): FlatCallNode[];
export function layoutNodes(flat: FlatCallNode[]): PositionedCallNode[];
export function buildFlowElements(graph: CallGraphInput | null | undefined): {
  nodes: FlowNode[];
  edges: FlowEdge[];
};
export function formatGas(gas: number | null | undefined): string;
export function callStackFor(
  nodes: FlowNode[],
  edges: FlowEdge[],
  nodeId: string,
): FlowNode[];
export function totalGas(nodes: FlowNode[]): number;
