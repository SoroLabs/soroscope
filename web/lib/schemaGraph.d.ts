import type { CallNode, ResourceReport } from './sorobantypes';

export declare const NODE_KIND: {
  CONTRACT: 'contract';
  STORAGE: 'storage';
};

export declare const COLUMN_WIDTH: number;
export declare const ROW_HEIGHT: number;

export type SchemaNodeKind = 'contract' | 'storage';

export interface SchemaContractNodeData {
  kind: 'contract';
  label: string;
  contractId: string;
  shortContractId: string;
  functionName: string;
  depth: number;
  isRoot: boolean;
}

export interface SchemaStorageNodeData {
  kind: 'storage';
  label: string;
  storageKey: string;
  source: string;
  written: boolean;
  remainingLedgers: number | null;
}

export type SchemaNodeData = SchemaContractNodeData | SchemaStorageNodeData;

export interface SchemaNode {
  id: string;
  type: SchemaNodeKind;
  position: { x: number; y: number };
  data: SchemaNodeData;
}

export interface SchemaEdge {
  id: string;
  source: string;
  target: string;
  animated: boolean;
  label: string;
  data: { kind: 'call' | 'storage'; isCycle?: boolean; written?: boolean };
}

export interface SchemaGraphStats {
  contractNodes: number;
  storageNodes: number;
  hiddenStorageNodes: number;
  maxDepth: number;
  hasCycle: boolean;
}

export interface SchemaGraph {
  nodes: SchemaNode[];
  edges: SchemaEdge[];
  stats: SchemaGraphStats;
}

export interface BuildSchemaGraphOptions {
  /** Render ledger/TTL storage keys alongside the call tree. Default `true`. */
  includeStorage?: boolean;
  /** Cap on rendered storage nodes. Default `12`. */
  maxStorageNodes?: number;
}

export declare function truncateContractId(
  contractId: string,
  lead?: number,
  tail?: number,
): string;

export declare function flattenCallGraph(root: CallNode | null | undefined): Array<{
  id: string;
  contractId: string;
  functionName: string;
  depth: number;
  parentId: string | null;
  isCycle: boolean;
}>;

export declare function buildSchemaGraph(
  report: ResourceReport | null | undefined,
  options?: BuildSchemaGraphOptions,
): SchemaGraph;

export declare function hasSchemaGraphData(report: ResourceReport | null | undefined): boolean;
