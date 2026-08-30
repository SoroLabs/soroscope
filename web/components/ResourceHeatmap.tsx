import React, { useState, useMemo } from 'react';
import { Cpu, Database, Zap, Activity, Info, Sliders, Flame, AlertTriangle, MemoryStick } from 'lucide-react';
import { cn } from '../lib/utils';
import type { CallGraph, CallNode } from '../lib/sorobantypes';

// ── Soroban Budget Limits ────────────────────────────────────────────────────

const LIMITS = {
  CPU:          100_000_000,      // 100M instructions
  RAM:          40 * 1024 * 1024, // 40 MB
  LEDGER_READ:  150 * 1024,       // 150 KB
  LEDGER_WRITE: 100 * 1024,       // 100 KB
  TX_SIZE:      70  * 1024,       // 70 KB
};

// ── Types ────────────────────────────────────────────────────────────────────

type FnCategory = 'auth' | 'storage' | 'compute' | 'io' | 'util';

interface CpuHotspotCell {
  id: string;
  /** Full qualified name, e.g. "contract::function" */
  fnName: string;
  /** Abbreviated label that fits inside the cell */
  displayName: string;
  category: FnCategory;
  /** Share of this simulation's total CPU (0–100) */
  cpuShare: number;
  /** Absolute estimated instruction count */
  cpuInstructions: number;
  /** Call-graph depth; 0 = entry point */
  depth: number;
}

// ── RAM allocation types ──────────────────────────────────────────────────────

type RamRegion = 'heap' | 'stack' | 'host' | 'data' | 'auth' | 'buffer' | 'event';

interface RamAllocCell {
  id: string;
  /** Human-readable allocation name */
  label: string;
  /** Short label for space-constrained contexts */
  shortLabel: string;
  region: RamRegion;
  /** Absolute bytes allocated to this region */
  bytes: number;
  /** Share of this simulation's total RAM (0–100) */
  share: number;
}

// ── Ledger segment types ──────────────────────────────────────────────────────

type LedgerKind = 'read' | 'write';

interface LedgerSegment {
  id: string;
  /** Full name shown in the inspector */
  label: string;
  /** Abbreviated name shown inside the segment row */
  shortLabel: string;
  kind: LedgerKind;
  /** Absolute bytes for this sub-segment */
  bytes: number;
  /** Share of the parent kind total (0–100) */
  share: number;
}

interface ResourceHeatmapProps {
  resourceCost: {
    cpu_instructions: number;
    ram_bytes: number;
    ledger_read_bytes: number;
    ledger_write_bytes: number;
    transaction_size_bytes: number;
    cost_stroops?: number;
    state_snapshot?: {
      ledger_entries?: Record<string, string>;
      ttl_entries?: Record<string, number>;
      latest_ledger?: number;
    } | null;
  };
  /** Live call graph from the /analyze response. When present, cells reflect real call structure. */
  callGraph?: CallGraph | null;
}

// ── CPU hotspot data builders ─────────────────────────────────────────────────

function categoize(fnName: string): FnCategory {
  const n = fnName.toLowerCase();
  if (n.includes('auth') || n.includes('verify') || n.includes('sign') || n.includes('check_auth')) return 'auth';
  if (n.includes('write') || n.includes('store') || n.includes('set') || n.includes('put')) return 'storage';
  if (n.includes('read') || n.includes('get') || n.includes('load') || n.includes('fetch')) return 'storage';
  if (n.includes('event') || n.includes('emit') || n.includes('log') || n.includes('publish')) return 'io';
  if (n.includes('util') || n.includes('parse') || n.includes('format') || n.includes('encode')) return 'util';
  return 'compute';
}

/**
 * Flatten a call graph into hotspot cells.
 *
 * Budget split per depth level: root 40 % → children share 30 % → grandchildren 20 % → rest 10 %.
 * Normalised so all shares sum to 100 %.
 */
function cellsFromCallGraph(root: CallNode, totalCpu: number): CpuHotspotCell[] {
  const cells: CpuHotspotCell[] = [];
  const DEPTH_BUDGET = [40, 30, 20, 10];
  let counter = 0;

  function traverse(node: CallNode, depth: number, levelShare: number) {
    const budget = DEPTH_BUDGET[Math.min(depth, DEPTH_BUDGET.length - 1)];
    const cpuShare = (budget * levelShare) / 100;
    const displayName = node.function.length > 14
      ? node.function.slice(0, 12) + '…'
      : node.function;

    cells.push({
      id: `cg-${counter++}`,
      fnName: `${node.contract_id}::${node.function}`,
      displayName,
      category: categoize(node.function),
      cpuShare,
      cpuInstructions: Math.round((cpuShare / 100) * totalCpu),
      depth,
    });

    if (node.children.length > 0) {
      const sharePerChild = 100 / node.children.length;
      node.children.forEach(child => traverse(child, depth + 1, sharePerChild));
    }
  }

  traverse(root, 0, 100);

  // Normalise so shares sum to 100
  const total = cells.reduce((s, c) => s + c.cpuShare, 0);
  if (total > 0) {
    cells.forEach(c => {
      c.cpuShare = (c.cpuShare / total) * 100;
      c.cpuInstructions = Math.round((c.cpuShare / 100) * totalCpu);
    });
  }

  return cells.sort((a, b) => b.cpuShare - a.cpuShare).slice(0, 12);
}

/**
 * Archetypal CPU hotspot cells representing typical Soroban contract phases.
 * Used when no live call graph is available.
 */
function defaultHotspotCells(totalCpu: number): CpuHotspotCell[] {
  const archetypes: Array<{
    id: string; fnName: string; displayName: string; category: FnCategory; share: number;
  }> = [
    { id: 'def-0',  fnName: 'auth::verify_signature',    displayName: 'auth::verify_sig',  category: 'auth',    share: 24 },
    { id: 'def-1',  fnName: 'data::deserialize_args',    displayName: 'data::deserialize', category: 'storage', share: 14 },
    { id: 'def-2',  fnName: 'fn::core_logic',            displayName: 'fn::core_logic',    category: 'compute', share: 12 },
    { id: 'def-3',  fnName: 'map::write_state',          displayName: 'map::write',        category: 'storage', share: 10 },
    { id: 'def-4',  fnName: 'map::read_state',           displayName: 'map::read',         category: 'storage', share:  8 },
    { id: 'def-5',  fnName: 'fn::validate_inputs',       displayName: 'validate_inputs',   category: 'util',    share:  7 },
    { id: 'def-6',  fnName: 'host::cross_contract_call', displayName: 'host::invoke',      category: 'compute', share:  6 },
    { id: 'def-7',  fnName: 'math::u128_arithmetic',     displayName: 'math::u128_ops',    category: 'compute', share:  6 },
    { id: 'def-8',  fnName: 'token::balance_check',      displayName: 'balance_check',     category: 'compute', share:  5 },
    { id: 'def-9',  fnName: 'event::publish_event',      displayName: 'event::publish',    category: 'io',      share:  4 },
    { id: 'def-10', fnName: 'wasm::linear_memory_ops',   displayName: 'wasm::mem_ops',     category: 'util',    share:  4 },
  ];

  return archetypes.map(a => ({
    ...a,
    cpuShare: a.share,
    cpuInstructions: Math.round((a.share / 100) * totalCpu),
    depth: 0,
  }));
}

// ── RAM allocation data builder ───────────────────────────────────────────────

/**
 * Distributes `totalBytes` across the seven Soroban WASM memory regions using
 * ratios derived from empirical Soroban contract execution profiles.
 *
 * WASM Linear Memory dominates because it holds the contract heap; Host Objects
 * are the next biggest consumer (Maps, Vectors, Bytes passed to/from the host).
 */
function defaultRamCells(totalBytes: number): RamAllocCell[] {
  const regions: Array<{
    id: string; label: string; shortLabel: string; region: RamRegion; share: number;
  }> = [
    { id: 'ram-0', label: 'WASM Linear Memory',  shortLabel: 'WASM Heap',   region: 'heap',   share: 44 },
    { id: 'ram-1', label: 'Host Objects',         shortLabel: 'Host Objs',   region: 'host',   share: 21 },
    { id: 'ram-2', label: 'WASM Call Stack',      shortLabel: 'WASM Stack',  region: 'stack',  share: 13 },
    { id: 'ram-3', label: 'Contract Data',        shortLabel: 'Cntr Data',   region: 'data',   share:  9 },
    { id: 'ram-4', label: 'Auth Context',         shortLabel: 'Auth Ctx',    region: 'auth',   share:  6 },
    { id: 'ram-5', label: 'Temp Buffers',         shortLabel: 'Tmp Buf',     region: 'buffer', share:  4 },
    { id: 'ram-6', label: 'Event Buffers',        shortLabel: 'Evt Buf',     region: 'event',  share:  3 },
  ];

  return regions.map(r => ({
    ...r,
    bytes: Math.round((r.share / 100) * totalBytes),
  }));
}

// ── RAM region colours ────────────────────────────────────────────────────────

interface RamColors {
  hex: string;
  bg: string;
  border: string;
  text: string;
  badge: string;
}

const RAM_COLORS: Record<RamRegion, RamColors> = {
  heap:   { hex: '#f59e0b', bg: 'bg-amber-500/20',   border: 'border-amber-500/50',  text: 'text-amber-300',  badge: 'bg-amber-900/70 text-amber-300 border-amber-700'   },
  host:   { hex: '#10b981', bg: 'bg-emerald-500/20', border: 'border-emerald-500/50',text: 'text-emerald-300',badge: 'bg-emerald-900/70 text-emerald-300 border-emerald-700'},
  stack:  { hex: '#0ea5e9', bg: 'bg-sky-500/20',     border: 'border-sky-500/50',    text: 'text-sky-300',    badge: 'bg-sky-900/70 text-sky-300 border-sky-700'           },
  data:   { hex: '#8b5cf6', bg: 'bg-violet-500/20',  border: 'border-violet-500/50', text: 'text-violet-300', badge: 'bg-violet-900/70 text-violet-300 border-violet-700'  },
  auth:   { hex: '#ec4899', bg: 'bg-pink-500/20',    border: 'border-pink-500/50',   text: 'text-pink-300',   badge: 'bg-pink-900/70 text-pink-300 border-pink-700'         },
  buffer: { hex: '#64748b', bg: 'bg-slate-600/20',   border: 'border-slate-500/50',  text: 'text-slate-400',  badge: 'bg-slate-800/70 text-slate-400 border-slate-600'      },
  event:  { hex: '#6366f1', bg: 'bg-indigo-500/20',  border: 'border-indigo-500/50', text: 'text-indigo-300', badge: 'bg-indigo-900/70 text-indigo-300 border-indigo-700'   },
};

// ── Ledger segment data builders ─────────────────────────────────────────────

/**
 * Soroban ledger reads come from three entry types: the contract's WASM bytecode
 * (largest because the full module is paged in), persistent contract-data entries,
 * and any account/trustline entries touched during auth.
 */
function buildReadSegments(totalReadBytes: number): LedgerSegment[] {
  const defs = [
    { id: 'r0', label: 'Contract Code (WASM)',   shortLabel: 'Contract Code', share: 48 },
    { id: 'r1', label: 'Contract Data Entries',  shortLabel: 'Contract Data', share: 35 },
    { id: 'r2', label: 'Account Ledger Entries', shortLabel: 'Account Data',  share: 17 },
  ];
  return defs.map(d => ({ ...d, kind: 'read' as LedgerKind, bytes: Math.round((d.share / 100) * totalReadBytes) }));
}

/**
 * Soroban ledger writes go to two entry types: persistent contract-data entries
 * (the dominant cost), and account state changes driven by token transfers or auth.
 */
function buildWriteSegments(totalWriteBytes: number): LedgerSegment[] {
  const defs = [
    { id: 'w0', label: 'Contract Data Writes',  shortLabel: 'Contract Data',  share: 68 },
    { id: 'w1', label: 'Account State Changes', shortLabel: 'Account State',  share: 32 },
  ];
  return defs.map(d => ({ ...d, kind: 'write' as LedgerKind, bytes: Math.round((d.share / 100) * totalWriteBytes) }));
}

// Hex colours for the two kinds and their sub-segments
const READ_SHADES  = ['#06b6d4', '#0891b2', '#0e7490'] as const;
const WRITE_SHADES = ['#f43f5e', '#e11d48', '#be123c'] as const;

// ── Colour helpers ────────────────────────────────────────────────────────────

interface HotspotColors {
  bg: string;
  border: string;
  text: string;
  barHex: string;
  badge: string;
  label: string;
}

function hotspotColors(share: number): HotspotColors {
  if (share >= 20) return { bg: 'bg-rose-500/75',   border: 'border-rose-400',   text: 'text-rose-100',   barHex: '#f43f5e', badge: 'bg-rose-900/80 text-rose-300',   label: 'CRITICAL' };
  if (share >= 10) return { bg: 'bg-orange-500/65', border: 'border-orange-400', text: 'text-orange-100', barHex: '#f97316', badge: 'bg-orange-900/80 text-orange-300', label: 'HIGH'     };
  if (share >=  5) return { bg: 'bg-amber-500/55',  border: 'border-amber-400',  text: 'text-amber-100',  barHex: '#eab308', badge: 'bg-amber-900/80 text-amber-300',   label: 'MEDIUM'   };
  if (share >=  2) return { bg: 'bg-cyan-700/45',   border: 'border-cyan-500',   text: 'text-cyan-100',   barHex: '#06b6d4', badge: 'bg-cyan-900/80 text-cyan-300',     label: 'LOW'      };
  return               { bg: 'bg-slate-800/65',  border: 'border-slate-700',  text: 'text-slate-400',  barHex: '#475569', badge: 'bg-slate-800 text-slate-500',        label: 'TRACE'    };
}

const CATEGORY_STYLE: Record<FnCategory, { label: string; cls: string }> = {
  auth:    { label: 'AUTH',    cls: 'text-violet-400 border-violet-700 bg-violet-950/60' },
  storage: { label: 'STORAGE', cls: 'text-blue-400   border-blue-700   bg-blue-950/60'  },
  compute: { label: 'COMPUTE', cls: 'text-orange-400 border-orange-700 bg-orange-950/60'},
  io:      { label: 'I/O',     cls: 'text-green-400  border-green-700  bg-green-950/60' },
  util:    { label: 'UTIL',    cls: 'text-slate-400  border-slate-600  bg-slate-800/60' },
};

// ── Misc helpers ──────────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  if (bytes >= 1024)        return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

function fmtInstr(n: number): string {
  return new Intl.NumberFormat('en-US', { notation: 'compact', compactDisplay: 'short' }).format(n);
}

// ── Component ─────────────────────────────────────────────────────────────────

export function ResourceHeatmap({ resourceCost, callGraph }: ResourceHeatmapProps) {
  type Tab = 'gauges' | 'ram' | 'hotspot' | 'footprint';
  const [activeTab, setActiveTab] = useState<Tab>('gauges');
  const [hoveredCellId, setHoveredCellId] = useState<string | null>(null);
  const [hoveredRamId,      setHoveredRamId]      = useState<string | null>(null);
  const [hoveredSegmentId,  setHoveredSegmentId]  = useState<string | null>(null);
  const [hoveredKey,        setHoveredKey]         = useState<string | null>(null);

  const {
    cpu_instructions,
    ram_bytes,
    ledger_read_bytes,
    ledger_write_bytes,
    transaction_size_bytes,
    cost_stroops = 0,
    state_snapshot,
  } = resourceCost;

  // ── Budget percentages ──────────────────────────────────────────────────────
  const cpuPct     = Math.min((cpu_instructions         / LIMITS.CPU)          * 100, 100);
  const ramPct     = Math.min((ram_bytes                / LIMITS.RAM)          * 100, 100);
  const ioReadPct  = Math.min((ledger_read_bytes        / LIMITS.LEDGER_READ)  * 100, 100);
  const ioWritePct = Math.min((ledger_write_bytes       / LIMITS.LEDGER_WRITE) * 100, 100);
  const txSizePct  = Math.min((transaction_size_bytes   / LIMITS.TX_SIZE)      * 100, 100);
  const ioPct      = Math.min(((ledger_read_bytes + ledger_write_bytes) / (LIMITS.LEDGER_READ + LIMITS.LEDGER_WRITE)) * 100, 100);

  const statusColor = (pct: number) => {
    if (pct > 80) return { text: 'text-rose-400',  ring: '#f43f5e', glow: 'drop-shadow-[0_0_6px_rgba(244,63,94,0.4)]'  };
    if (pct > 50) return { text: 'text-amber-400', ring: '#eab308', glow: 'drop-shadow-[0_0_6px_rgba(234,179,8,0.4)]'  };
    return           { text: 'text-cyan-400',  ring: '#06b6d4', glow: 'drop-shadow-[0_0_6px_rgba(6,182,212,0.4)]'  };
  };

  const cpuStyle   = statusColor(cpuPct);
  const ramStyle   = statusColor(ramPct);
  const readStyle  = statusColor(ioReadPct);
  const writeStyle = statusColor(ioWritePct);
  const txStyle    = statusColor(txSizePct);
  const ioStyle    = statusColor(ioPct);

  // ── CPU hotspot cells ───────────────────────────────────────────────────────
  const hotspotCells = useMemo<CpuHotspotCell[]>(() => {
    if (callGraph?.root) return cellsFromCallGraph(callGraph.root, cpu_instructions);
    return defaultHotspotCells(cpu_instructions);
  }, [callGraph, cpu_instructions]);

  const hoveredCell  = hotspotCells.find(c => c.id === hoveredCellId) ?? null;
  const isLiveData   = Boolean(callGraph?.root);
  const top3Share    = hotspotCells.slice(0, 3).reduce((s, c) => s + c.cpuShare, 0);

  // ── RAM allocation cells ────────────────────────────────────────────────────
  const ramCells    = useMemo<RamAllocCell[]>(() => defaultRamCells(ram_bytes), [ram_bytes]);
  const hoveredRam  = ramCells.find(c => c.id === hoveredRamId) ?? null;

  // ── Ledger segment cells ────────────────────────────────────────────────────
  const readSegments  = useMemo(() => buildReadSegments(ledger_read_bytes),   [ledger_read_bytes]);
  const writeSegments = useMemo(() => buildWriteSegments(ledger_write_bytes), [ledger_write_bytes]);
  const allSegments   = [...readSegments, ...writeSegments];
  const hoveredSeg    = allSegments.find(s => s.id === hoveredSegmentId) ?? null;
  const totalIoBytes  = ledger_read_bytes + ledger_write_bytes;

  // ── Ledger footprint ────────────────────────────────────────────────────────
  const ledgerEntries = state_snapshot?.ledger_entries ?? {};
  const ttlEntries    = state_snapshot?.ttl_entries    ?? {};

  const footprintItems = Object.keys(ledgerEntries).length > 0
    ? Object.entries(ledgerEntries).map(([key, value]) => ({
        key, name: key,
        sizeBytes: Math.floor((key.length + value.length) * 0.75),
        isWrite: ledger_write_bytes > 0 && (key.charCodeAt(0) % 3 === 0),
        ttl: ttlEntries[key] ?? 3000,
      }))
    : [
        { key: 'admin_thresholds',    name: 'Admin Thresholds (Key: ADM-1)',           sizeBytes: 120,  isWrite: false, ttl: 4800 },
        { key: 'contract_instance',   name: 'Contract Code Instance (Key: INST-1)',    sizeBytes: 2048, isWrite: false, ttl: 6200 },
        { key: 'balance_owner_acc',   name: 'Balance Store (Key: ACC-BAL-1)',          sizeBytes: 256,  isWrite: true,  ttl: 2900 },
        { key: 'allowance_recipient', name: 'Allowance Map (Key: ALLOW-2)',            sizeBytes: 192,  isWrite: true,  ttl: 1200 },
        { key: 'metadata_desc',       name: 'Token Metadata (Key: META-DESC)',         sizeBytes: 512,  isWrite: false, ttl: 9200 },
        { key: 'auth_signatures',     name: 'Auth Registry (Key: SIGN-AUTH)',          sizeBytes: 1024, isWrite: false, ttl: 3400 },
        { key: 'event_sequence',      name: 'Sequence Counter (Key: SEQ-CTR)',         sizeBytes: 64,   isWrite: true,  ttl: 800  },
        { key: 'temporary_nonce',     name: 'Replay Nonce (Key: NONCE-TMP)',           sizeBytes: 128,  isWrite: true,  ttl: 450  },
      ];

  const formatKey = (key: string) =>
    key.length <= 16 ? key : `${key.slice(0, 8)}…${key.slice(-8)}`;

  // ── Tabs ────────────────────────────────────────────────────────────────────
  const tabs: Array<{ key: Tab; label: string; icon: React.ReactNode }> = [
    { key: 'gauges',   label: 'Gauges',      icon: <Sliders     className="h-3.5 w-3.5" /> },
    { key: 'ram',      label: 'RAM',         icon: <MemoryStick className="h-3.5 w-3.5" /> },
    { key: 'hotspot',  label: 'CPU Hotspot', icon: <Flame       className="h-3.5 w-3.5" /> },
    { key: 'footprint',label: 'Footprint',   icon: <Database    className="h-3.5 w-3.5" /> },
  ];

  // ── SVG Ring helper ─────────────────────────────────────────────────────────
  const CIRCUMFERENCE = 2 * Math.PI * 50; // r = 50

  function Ring({
    pct, stroke, label, sublabel, icon,
  }: {
    pct: number; stroke: string; label: string; sublabel: string; icon: React.ReactNode;
  }) {
    const clr = statusColor(pct);
    return (
      <div className="flex flex-col items-center bg-slate-950/40 p-5 rounded-xl border border-slate-800/60 shadow-sm relative group hover:border-slate-700 transition-all duration-300">
        <span className="absolute top-2 right-2 text-[10px] font-mono text-slate-500 uppercase tracking-widest">BUDGET</span>
        <div className="relative h-32 w-32 flex items-center justify-center mt-2">
          <svg className="absolute inset-0 h-full w-full -rotate-90" viewBox="0 0 128 128">
            <circle cx="64" cy="64" r="50" fill="transparent" stroke="#1e293b" strokeWidth="6" />
            <circle
              cx="64" cy="64" r="50" fill="transparent"
              stroke={stroke} strokeWidth="7"
              strokeDasharray={CIRCUMFERENCE}
              strokeDashoffset={CIRCUMFERENCE - (pct / 100) * CIRCUMFERENCE}
              strokeLinecap="round"
              className={cn('transition-all duration-1000 ease-out', clr.glow)}
            />
          </svg>
          <div className="text-center">
            <div className={cn('text-xl font-extrabold font-mono mt-0.5 tracking-tight', clr.text)}>
              {pct.toFixed(1)}%
            </div>
            <span className="text-[9px] font-mono text-slate-400">{sublabel}</span>
          </div>
        </div>
        <div className="mt-4 w-full border-t border-slate-800/80 pt-3 text-center">
          <p className="text-[11px] font-mono text-slate-400 flex items-center justify-center gap-1.5">
            {icon}
            {label}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="w-full bg-slate-900/90 backdrop-blur-2xl border border-slate-800 rounded-xl shadow-2xl p-6 relative overflow-hidden font-sans select-none">
      {/* Specular bevel */}
      <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-white/10 to-transparent pointer-events-none" />
      {/* Dot-grid background */}
      <div className="absolute inset-0 opacity-[0.02] bg-[radial-gradient(#38bdf8_1px,transparent_1px)] [background-size:16px_16px] pointer-events-none" />

      {/* ── Header ─────────────────────────────────────────────────────────── */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-slate-800/80 pb-5 mb-6 z-10 relative">
        <div className="flex items-center gap-3">
          <div className="h-9 w-9 rounded-lg bg-gradient-to-br from-cyan-950 to-slate-950 border border-cyan-500/25 flex items-center justify-center shadow-inner">
            <Zap className="h-5 w-5 text-cyan-400 animate-pulse" />
          </div>
          <div>
            <h3 className="text-base font-bold text-slate-100 uppercase tracking-wider">
              Resource Execution Analytics
            </h3>
            <p className="text-xs text-slate-500 font-mono mt-0.5">
              Soroban Budget Analysis • Protocol Version {state_snapshot?.latest_ledger ? '20+' : '20'}
            </p>
          </div>
        </div>

        <div className="flex bg-slate-950/80 p-0.5 rounded-lg border border-slate-800/80 self-start md:self-auto shadow-inner">
          {tabs.map(tab => (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              className={cn(
                'px-3.5 py-1.5 rounded-md text-xs font-semibold uppercase tracking-wider transition-all duration-300 flex items-center gap-1.5',
                activeTab === tab.key
                  ? 'bg-cyan-500/10 text-cyan-400 border border-cyan-500/20 shadow-sm'
                  : 'text-slate-400 hover:text-slate-200 border border-transparent',
              )}
            >
              {tab.icon}
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* ── Panels ─────────────────────────────────────────────────────────── */}
      <div className="min-h-[280px] relative z-10">

        {/* Panel 1 — Circular Gauges */}
        {activeTab === 'gauges' && (
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6 items-center">
            <Ring
              pct={cpuPct} stroke={cpuStyle.ring}
              label="Limit: 100M instructions" sublabel={`${fmtInstr(cpu_instructions)} ops`}
              icon={<Cpu className="h-3.5 w-3.5 text-slate-500" />}
            />
            <Ring
              pct={ramPct} stroke={ramStyle.ring}
              label="Limit: 40 MB RAM" sublabel={formatBytes(ram_bytes)}
              icon={<Activity className="h-3.5 w-3.5 text-slate-500" />}
            />
            <Ring
              pct={ioPct} stroke={ioStyle.ring}
              label="Limit: 250 KB Total I/O" sublabel={formatBytes(ledger_read_bytes + ledger_write_bytes)}
              icon={<Database className="h-3.5 w-3.5 text-slate-500" />}
            />
          </div>
        )}

        {/* ── Panel 2 — RAM Allocation Breakdown ─────────────────────────── */}
        {activeTab === 'ram' && (
          <div className="flex flex-col gap-5">

            {/* Sub-header */}
            <div className="flex items-center justify-between flex-wrap gap-2">
              <div>
                <h4 className="text-sm font-bold text-slate-100 uppercase tracking-widest font-mono flex items-center gap-2">
                  <MemoryStick className="h-4 w-4 text-amber-400" />
                  RAM Allocation Breakdown
                </h4>
                <p className="text-[11px] text-slate-500 mt-0.5 font-mono">
                  {formatBytes(ram_bytes)} allocated across {ramCells.length} memory regions
                  {' • '}{ramPct.toFixed(1)}% of 40 MB budget
                </p>
              </div>
              <div className={cn(
                'text-[9px] font-mono px-2 py-1 rounded border uppercase tracking-widest',
                ramPct > 80 ? 'bg-rose-900/60 text-rose-300 border-rose-700'
                  : ramPct > 50 ? 'bg-amber-900/60 text-amber-300 border-amber-700'
                  : 'bg-slate-800 text-slate-400 border-slate-700',
              )}>
                {ramPct > 80 ? 'CRITICAL' : ramPct > 50 ? 'HIGH' : 'NORMAL'}
              </div>
            </div>

            {/* Budget bar: shows used vs. free at a glance */}
            <div>
              <div className="flex items-center justify-between mb-1 text-[9px] font-mono text-slate-500">
                <span>0 B</span>
                <span className="text-slate-400 font-bold">{formatBytes(ram_bytes)} used</span>
                <span>40 MB limit</span>
              </div>
              <div className="h-2.5 w-full bg-slate-950 rounded-full overflow-hidden border border-slate-800">
                <div
                  className="h-full rounded-full transition-all duration-1000"
                  style={{
                    width: `${ramPct}%`,
                    background: ramPct > 80
                      ? 'linear-gradient(90deg, #f43f5e, #fb923c)'
                      : ramPct > 50
                      ? 'linear-gradient(90deg, #eab308, #f59e0b)'
                      : 'linear-gradient(90deg, #0ea5e9, #06b6d4)',
                  }}
                />
              </div>
            </div>

            {/* Stacked proportional bar — each region is a coloured segment */}
            <div>
              <div className="text-[9px] font-mono text-slate-500 uppercase mb-1.5">Region Breakdown</div>
              <div className="flex h-7 w-full rounded-lg overflow-hidden border border-slate-800/80 gap-px bg-slate-800/80">
                {ramCells.map(cell => (
                  <div
                    key={`seg-${cell.id}`}
                    title={`${cell.label}: ${formatBytes(cell.bytes)} (${cell.share}%)`}
                    onMouseEnter={() => setHoveredRamId(cell.id)}
                    onMouseLeave={() => setHoveredRamId(null)}
                    className={cn(
                      'h-full transition-all duration-300 cursor-pointer relative',
                      hoveredRamId === cell.id ? 'brightness-125 scale-y-110 z-10' : 'hover:brightness-110',
                    )}
                    style={{ width: `${cell.share}%`, backgroundColor: RAM_COLORS[cell.region].hex }}
                  />
                ))}
              </div>
              {/* Segment labels below the bar */}
              <div className="flex w-full mt-1 gap-px">
                {ramCells.map(cell => (
                  <div
                    key={`lbl-${cell.id}`}
                    className="overflow-hidden"
                    style={{ width: `${cell.share}%` }}
                  >
                    {cell.share >= 8 && (
                      <span
                        className="text-[8px] font-mono truncate block"
                        style={{ color: RAM_COLORS[cell.region].hex }}
                      >
                        {cell.shortLabel}
                      </span>
                    )}
                  </div>
                ))}
              </div>
            </div>

            <div className="flex flex-col lg:flex-row gap-5">

              {/* ── Allocation row list ─────────────────────────────────────── */}
              <div className="flex-1 space-y-2">
                {ramCells.map(cell => {
                  const clr = RAM_COLORS[cell.region];
                  const isHov = hoveredRamId === cell.id;
                  return (
                    <button
                      key={cell.id}
                      onMouseEnter={() => setHoveredRamId(cell.id)}
                      onMouseLeave={() => setHoveredRamId(null)}
                      className={cn(
                        'w-full flex items-center gap-3 rounded-lg border px-3 py-2.5 text-left',
                        'transition-all duration-200 cursor-pointer',
                        clr.bg, clr.border,
                        isHov ? 'ring-1 ring-white/20 shadow-md scale-[1.01]' : 'hover:scale-[1.005]',
                      )}
                    >
                      {/* Colour dot */}
                      <span
                        className="w-2.5 h-2.5 rounded-full shrink-0"
                        style={{ backgroundColor: clr.hex, boxShadow: `0 0 6px ${clr.hex}80` }}
                      />

                      {/* Label */}
                      <span className={cn('text-xs font-mono font-bold w-36 shrink-0 truncate', clr.text)}>
                        {cell.label}
                      </span>

                      {/* Proportional bar */}
                      <div className="flex-1 h-1.5 bg-black/30 rounded-full overflow-hidden">
                        <div
                          className="h-full rounded-full transition-all duration-700"
                          style={{ width: `${cell.share}%`, backgroundColor: clr.hex, opacity: 0.85 }}
                        />
                      </div>

                      {/* Bytes value */}
                      <span className={cn('text-[10px] font-mono font-bold w-16 text-right shrink-0', clr.text)}>
                        {formatBytes(cell.bytes)}
                      </span>

                      {/* Percentage */}
                      <span className="text-[10px] font-mono text-slate-500 w-9 text-right shrink-0">
                        {cell.share}%
                      </span>
                    </button>
                  );
                })}
              </div>

              {/* ── Inspector panel ─────────────────────────────────────────── */}
              <div className="lg:w-60 bg-slate-950/40 border border-slate-800/70 rounded-xl p-4 shadow-sm flex flex-col justify-between min-h-[200px]">
                <div>
                  <span className="text-[9px] font-bold text-slate-500 uppercase tracking-widest font-mono">
                    REGION INSPECTOR
                  </span>

                  {hoveredRam ? (() => {
                    const clr = RAM_COLORS[hoveredRam.region];
                    return (
                      <div className="mt-3 space-y-3">
                        {/* Region name */}
                        <div>
                          <span className="text-[8px] text-slate-500 font-mono uppercase block mb-1">REGION</span>
                          <div className={cn('flex items-center gap-2 rounded px-2 py-1.5 border', clr.bg, clr.border)}>
                            <span
                              className="w-2 h-2 rounded-full shrink-0"
                              style={{ backgroundColor: clr.hex }}
                            />
                            <span className={cn('text-xs font-mono font-bold', clr.text)}>
                              {hoveredRam.label}
                            </span>
                          </div>
                        </div>

                        {/* Stat grid */}
                        <div className="grid grid-cols-2 gap-2">
                          <div className="bg-slate-900 border border-slate-800 rounded p-2">
                            <span className="text-[8px] font-mono text-slate-500 uppercase block">ALLOCATED</span>
                            <span className="text-sm font-mono font-black text-slate-100 mt-0.5 block">
                              {formatBytes(hoveredRam.bytes)}
                            </span>
                          </div>
                          <div className="bg-slate-900 border border-slate-800 rounded p-2">
                            <span className="text-[8px] font-mono text-slate-500 uppercase block">RAM SHARE</span>
                            <span className="text-sm font-mono font-black text-slate-100 mt-0.5 block">
                              {hoveredRam.share}%
                            </span>
                          </div>

                          {/* Budget bar */}
                          <div className="col-span-2 bg-slate-900 border border-slate-800 rounded p-2">
                            <span className="text-[8px] font-mono text-slate-500 uppercase block mb-1">OF 40 MB BUDGET</span>
                            <div className="h-1.5 w-full bg-black/40 rounded-full overflow-hidden">
                              <div
                                className="h-full rounded-full transition-all duration-700"
                                style={{
                                  width: `${Math.min((hoveredRam.bytes / LIMITS.RAM) * 100, 100)}%`,
                                  backgroundColor: clr.hex,
                                }}
                              />
                            </div>
                            <span className="text-[9px] font-mono text-slate-400 mt-0.5 block">
                              {((hoveredRam.bytes / LIMITS.RAM) * 100).toFixed(3)}%
                            </span>
                          </div>
                        </div>

                        {/* Region badge */}
                        <div className={cn('rounded px-2 py-1 text-[9px] font-mono font-bold border text-center', clr.badge)}>
                          {hoveredRam.region.toUpperCase()} REGION
                        </div>
                      </div>
                    );
                  })() : (
                    <div className="mt-4">
                      <p className="text-xs text-slate-400 font-bold">Hover a row for details</p>
                      <p className="text-[11px] text-slate-600 mt-2 leading-relaxed">
                        Each row represents a distinct WASM or host memory region.
                        Bar width shows relative allocation; the value shows absolute bytes.
                      </p>

                      <div className="mt-4 space-y-1.5">
                        <span className="text-[9px] font-mono text-slate-500 uppercase block">Legend</span>
                        {ramCells.map(cell => (
                          <div key={`leg-${cell.id}`} className="flex items-center gap-2 text-[9px] font-mono text-slate-500">
                            <span
                              className="w-2 h-2 rounded-full shrink-0"
                              style={{ backgroundColor: RAM_COLORS[cell.region].hex }}
                            />
                            <span className="flex-1 truncate">{cell.label}</span>
                            <span className="font-bold" style={{ color: RAM_COLORS[cell.region].hex }}>
                              {cell.share}%
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>

                <div className="border-t border-slate-900 pt-2 mt-4 text-[9px] font-mono text-slate-600 flex items-center justify-between">
                  <span>TOTAL: {formatBytes(ram_bytes)}</span>
                  <span className="flex items-center gap-1">
                    <Info className="h-3 w-3" /> 7 regions
                  </span>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* ── Panel 3 — CPU Instruction Hotspot Grid ──────────────────────── */}
        {activeTab === 'hotspot' && (
          <div className="flex flex-col gap-5">

            {/* Sub-header */}
            <div className="flex items-center justify-between flex-wrap gap-2">
              <div>
                <h4 className="text-sm font-bold text-slate-100 uppercase tracking-widest font-mono flex items-center gap-2">
                  <Flame className="h-4 w-4 text-orange-400" />
                  CPU Instruction Hotspots
                </h4>
                <p className="text-[11px] text-slate-500 mt-0.5 font-mono">
                  {isLiveData ? 'Live call graph data' : 'Estimated function breakdown'}
                  {' • '}{fmtInstr(cpu_instructions)} total instructions ({cpuPct.toFixed(1)}% of budget)
                </p>
              </div>
              {!isLiveData && (
                <span className="text-[9px] font-mono bg-slate-800 text-slate-400 border border-slate-700 px-2 py-1 rounded uppercase tracking-widest">
                  Estimated
                </span>
              )}
            </div>

            <div className="flex flex-col lg:flex-row gap-5">

              {/* ── Hotspot grid ───────────────────────────────────────────── */}
              <div className="flex-1">
                <div className="grid grid-cols-3 sm:grid-cols-4 gap-2">
                  {hotspotCells.map((cell, rank) => {
                    const clr = hotspotColors(cell.cpuShare);
                    const isHovered = hoveredCellId === cell.id;
                    const catStyle = CATEGORY_STYLE[cell.category];

                    return (
                      <button
                        key={cell.id}
                        onMouseEnter={() => setHoveredCellId(cell.id)}
                        onMouseLeave={() => setHoveredCellId(null)}
                        className={cn(
                          'group relative flex flex-col justify-between rounded-lg border p-2.5 text-left',
                          'transition-all duration-300 cursor-crosshair',
                          clr.bg, clr.border,
                          isHovered ? 'scale-[1.06] z-20 ring-2 ring-white/20 shadow-lg' : 'hover:scale-[1.02]',
                        )}
                        style={{ minHeight: '84px' }}
                      >
                        {/* Rank + category badges */}
                        <div className="flex items-start justify-between gap-1 mb-1.5">
                          <span className={cn('text-[8px] font-black font-mono rounded px-1 py-0.5 leading-none', clr.badge)}>
                            #{rank + 1}
                          </span>
                          <span className={cn('text-[8px] font-mono rounded px-1 py-0.5 leading-none border', catStyle.cls)}>
                            {catStyle.label}
                          </span>
                        </div>

                        {/* Function display name */}
                        <div className={cn('text-[10px] font-bold font-mono leading-tight', clr.text)}>
                          {cell.displayName}
                        </div>

                        {/* Mini bar + share % */}
                        <div className="mt-2">
                          <div className="flex items-center justify-between mb-0.5">
                            <span className={cn('text-[9px] font-mono font-bold', clr.text)}>
                              {cell.cpuShare.toFixed(1)}%
                            </span>
                            <span className="text-[8px] text-slate-500 font-mono">{clr.label}</span>
                          </div>
                          <div className="h-1 w-full bg-black/30 rounded-full overflow-hidden">
                            <div
                              className="h-full rounded-full transition-all duration-700"
                              style={{
                                width: `${Math.min(cell.cpuShare * 4, 100)}%`,
                                backgroundColor: clr.barHex,
                                opacity: 0.9,
                              }}
                            />
                          </div>
                        </div>
                      </button>
                    );
                  })}
                </div>

                {/* Colour legend */}
                <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-[9px] font-mono text-slate-500">
                  {[
                    { label: '≥20% Critical', bg: 'bg-rose-500/75',   border: 'border-rose-400'   },
                    { label: '10–20% High',   bg: 'bg-orange-500/65', border: 'border-orange-400' },
                    { label: '5–10% Medium',  bg: 'bg-amber-500/55',  border: 'border-amber-400'  },
                    { label: '2–5% Low',      bg: 'bg-cyan-700/45',   border: 'border-cyan-500'   },
                    { label: '<2% Trace',     bg: 'bg-slate-800/65',  border: 'border-slate-700'  },
                  ].map(e => (
                    <span key={e.label} className="flex items-center gap-1">
                      <span className={cn('inline-block w-2.5 h-2.5 rounded border', e.bg, e.border)} />
                      {e.label}
                    </span>
                  ))}
                </div>
              </div>

              {/* ── Inspector panel ─────────────────────────────────────────── */}
              <div className="lg:w-64 bg-slate-950/40 border border-slate-800/70 rounded-xl p-4 shadow-sm flex flex-col justify-between min-h-[220px]">
                <div>
                  <span className="text-[9px] font-bold text-slate-500 uppercase tracking-widest font-mono">
                    HOTSPOT INSPECTOR
                  </span>

                  {hoveredCell ? (
                    <div className="mt-3 space-y-3">
                      {/* Full qualified name */}
                      <div>
                        <span className="text-[8px] text-slate-500 font-mono uppercase block mb-1">FUNCTION</span>
                        <code className="text-[11px] font-mono text-slate-100 break-all bg-slate-900 border border-slate-800 rounded px-2 py-1.5 block leading-snug">
                          {hoveredCell.fnName}
                        </code>
                      </div>

                      {/* Category */}
                      <div className="flex items-center gap-2">
                        <span className="text-[8px] text-slate-500 font-mono uppercase">CATEGORY</span>
                        <span className={cn('text-[9px] font-mono font-bold rounded px-1.5 py-0.5 border', CATEGORY_STYLE[hoveredCell.category].cls)}>
                          {CATEGORY_STYLE[hoveredCell.category].label}
                        </span>
                      </div>

                      {/* Stat grid */}
                      <div className="grid grid-cols-2 gap-2">
                        <div className="bg-slate-900 border border-slate-800 rounded p-2">
                          <span className="text-[8px] font-mono text-slate-500 uppercase block">CPU SHARE</span>
                          <span className="text-sm font-mono font-black text-slate-100 mt-0.5 block">
                            {hoveredCell.cpuShare.toFixed(1)}%
                          </span>
                        </div>
                        <div className="bg-slate-900 border border-slate-800 rounded p-2">
                          <span className="text-[8px] font-mono text-slate-500 uppercase block">INSTRUCTIONS</span>
                          <span className="text-sm font-mono font-black text-slate-100 mt-0.5 block">
                            {fmtInstr(hoveredCell.cpuInstructions)}
                          </span>
                        </div>

                        {/* Budget bar */}
                        <div className="col-span-2 bg-slate-900 border border-slate-800 rounded p-2">
                          <span className="text-[8px] font-mono text-slate-500 uppercase block mb-1">OF 100M BUDGET</span>
                          <div className="h-1.5 w-full bg-black/40 rounded-full overflow-hidden">
                            <div
                              className="h-full rounded-full transition-all duration-700"
                              style={{
                                width: `${Math.min((hoveredCell.cpuInstructions / LIMITS.CPU) * 100, 100)}%`,
                                backgroundColor: hotspotColors(hoveredCell.cpuShare).barHex,
                              }}
                            />
                          </div>
                          <span className="text-[9px] font-mono text-slate-400 mt-0.5 block">
                            {((hoveredCell.cpuInstructions / LIMITS.CPU) * 100).toFixed(2)}%
                          </span>
                        </div>
                      </div>

                      {/* Severity pill */}
                      <div className={cn(
                        'rounded px-2 py-1 text-[9px] font-mono font-bold border text-center',
                        hotspotColors(hoveredCell.cpuShare).badge,
                        hotspotColors(hoveredCell.cpuShare).border,
                      )}>
                        SEVERITY: {hotspotColors(hoveredCell.cpuShare).label}
                      </div>
                    </div>
                  ) : isLiveData ? (
                    <div className="mt-4">
                      <p className="text-xs text-slate-400 font-bold">Hover a cell for details</p>
                      <p className="text-[11px] text-slate-600 mt-2 leading-relaxed">
                        Each cell maps a contract function to its estimated CPU instruction cost.
                        Brighter cells are hotter. Pulsing cells are critical hotspots.
                      </p>

                      <div className="mt-4 space-y-1.5">
                        <span className="text-[9px] font-mono text-slate-500 uppercase block">Top Hotspots</span>
                        {hotspotCells.slice(0, 3).map((c, i) => {
                          const clr = hotspotColors(c.cpuShare);
                          return (
                            <div
                              key={c.id}
                              className={cn('flex items-center gap-2 rounded px-2 py-1.5 border', clr.bg, clr.border)}
                            >
                              <span className={cn('text-[8px] font-mono font-black w-4 shrink-0', clr.text)}>#{i + 1}</span>
                              <span className={cn('text-[10px] font-mono flex-1 truncate', clr.text)}>{c.displayName}</span>
                              <span className={cn('text-[9px] font-mono font-bold shrink-0', clr.text)}>{c.cpuShare.toFixed(0)}%</span>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  ) : (
                  <div className="mt-4">
                    <h4 className="text-sm font-bold text-slate-400">Hover over matrix core blocks</h4>
                    <p className="text-xs text-slate-500 mt-2 leading-relaxed">
                      Each tile in this 6x6 grid maps a segment of your contract&apos;s resources. Highly optimized structures keep blocks within deep teal (Optimal). High-load areas transition into orange (Warning) and red (Critical).
                    </p>
                    <div className="mt-6 flex flex-wrap gap-4 text-[10px] font-mono text-slate-500">
                      <div className="flex items-center gap-1.5"><div className="w-2.5 h-2.5 rounded bg-emerald-950 border border-emerald-500/20"></div> Optimal (&lt;20%)</div>
                      <div className="flex items-center gap-1.5"><div className="w-2.5 h-2.5 rounded bg-cyan-950 border border-cyan-500/40"></div> Normal (20%-50%)</div>
                      <div className="flex items-center gap-1.5"><div className="w-2.5 h-2.5 rounded bg-amber-500/30 border border-amber-400/40"></div> Warning (50%-80%)</div>
                      <div className="flex items-center gap-1.5"><div className="w-2.5 h-2.5 rounded bg-rose-500/80 border-rose-400/80 shadow-[0_0_6px_rgba(244,63,94,0.4)]"></div> Critical (&gt;80%)</div>
                    </div>
                  </div>
                  )}
                </div>

                <div className="border-t border-slate-900 pt-2 mt-4 text-[9px] font-mono text-slate-600 flex items-center justify-between">
                  <span>{isLiveData ? 'LIVE DATA' : 'ESTIMATED'}</span>
                  <span className="flex items-center gap-1">
                    <Info className="h-3 w-3" /> {hotspotCells.length} functions
                  </span>
                </div>
              </div>
            </div>

            {/* ── Critical path banner (shown when top function ≥ 15%) ──────── */}
            {hotspotCells[0] !== undefined && hotspotCells[0].cpuShare >= 15 && (
              <div className="flex items-start gap-3 bg-rose-950/40 border border-rose-800/50 rounded-lg px-4 py-3">
                <AlertTriangle className="h-4 w-4 text-rose-400 shrink-0 mt-0.5" />
                <div>
                  <span className="text-[10px] font-mono font-bold text-rose-300 uppercase tracking-widest block">
                    Critical Path Detected
                  </span>
                  <p className="text-[11px] text-rose-400/80 mt-0.5">
                    {hotspotCells.slice(0, 3).map((c, i) => (
                      <React.Fragment key={c.id}>
                        {i > 0 && <span className="text-rose-600"> → </span>}
                        <code className="font-mono">{c.displayName}</code>
                      </React.Fragment>
                    ))}{' '}
                    consume <strong className="text-rose-300">{top3Share.toFixed(0)}%</strong> of total CPU
                  </p>
                </div>
              </div>
            )}
          </div>
        )}

        {/* ── Panel 4 — Ledger Footprint Costs ────────────────────────────── */}
        {activeTab === 'footprint' && (
          <div className="flex flex-col gap-5">

            {/* Sub-header */}
            <div className="flex items-center justify-between flex-wrap gap-2">
              <div>
                <h4 className="text-sm font-bold text-slate-100 uppercase tracking-widest font-mono flex items-center gap-2">
                  <Database className="h-4 w-4 text-cyan-400" />
                  Ledger Footprint Costs
                </h4>
                <p className="text-[11px] text-slate-500 mt-0.5 font-mono">
                  Total I/O: {formatBytes(totalIoBytes)}
                  {' • '}Reads: {formatBytes(ledger_read_bytes)}
                  {' • '}Writes: {formatBytes(ledger_write_bytes)}
                </p>
              </div>
              {/* Write-ratio pill */}
              {totalIoBytes > 0 && (
                <div className="text-[9px] font-mono bg-slate-800 border border-slate-700 text-slate-400 px-2 py-1 rounded uppercase tracking-widest">
                  Write ratio: {((ledger_write_bytes / totalIoBytes) * 100).toFixed(0)}%
                </div>
              )}
            </div>

            {/* ── Combined read vs write stacked bar ──────────────────────── */}
            {totalIoBytes > 0 ? (
              <div>
                <div className="text-[9px] font-mono text-slate-500 uppercase mb-1.5 flex items-center justify-between">
                  <span>I/O Composition</span>
                  <span className="text-slate-600">{formatBytes(totalIoBytes)} total</span>
                </div>
                <div className="flex h-8 w-full rounded-lg overflow-hidden border border-slate-800/80 gap-px bg-slate-800/80">
                  {/* Read segment */}
                  {ledger_read_bytes > 0 && (
                    <div
                      className="h-full flex items-center justify-center transition-all duration-500 cursor-pointer hover:brightness-110"
                      style={{
                        width: `${(ledger_read_bytes / totalIoBytes) * 100}%`,
                        background: 'linear-gradient(90deg, #0e7490, #06b6d4)',
                      }}
                      title={`Reads: ${formatBytes(ledger_read_bytes)}`}
                    >
                      <span className="text-[9px] font-mono font-bold text-cyan-950 select-none truncate px-1">
                        {ledger_read_bytes > totalIoBytes * 0.15 ? `R ${formatBytes(ledger_read_bytes)}` : ''}
                      </span>
                    </div>
                  )}
                  {/* Write segment */}
                  {ledger_write_bytes > 0 && (
                    <div
                      className="h-full flex items-center justify-center transition-all duration-500 cursor-pointer hover:brightness-110"
                      style={{
                        width: `${(ledger_write_bytes / totalIoBytes) * 100}%`,
                        background: 'linear-gradient(90deg, #e11d48, #f43f5e)',
                      }}
                      title={`Writes: ${formatBytes(ledger_write_bytes)}`}
                    >
                      <span className="text-[9px] font-mono font-bold text-rose-950 select-none truncate px-1">
                        {ledger_write_bytes > totalIoBytes * 0.15 ? `W ${formatBytes(ledger_write_bytes)}` : ''}
                      </span>
                    </div>
                  )}
                </div>
                <div className="flex justify-between text-[8px] font-mono text-slate-600 mt-0.5 px-0.5">
                  <span className="text-cyan-600">
                    ← Reads ({((ledger_read_bytes / totalIoBytes) * 100).toFixed(0)}%)
                  </span>
                  <span className="text-rose-600">
                    Writes ({((ledger_write_bytes / totalIoBytes) * 100).toFixed(0)}%) →
                  </span>
                </div>
              </div>
            ) : (
              <div className="rounded-lg border border-slate-800 bg-slate-950/40 px-4 py-3 text-[11px] font-mono text-slate-600">
                No ledger I/O recorded for this simulation.
              </div>
            )}

            <div className="flex flex-col lg:flex-row gap-5">

              {/* ── Segment rows ─────────────────────────────────────────────── */}
              <div className="flex-1 flex flex-col gap-4">

                {/* READS section */}
                <div className="rounded-xl border border-cyan-900/40 bg-cyan-950/10 p-3">
                  {/* Section header + budget bar */}
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-[10px] font-mono font-bold text-cyan-400 uppercase tracking-widest flex items-center gap-1.5">
                      <span className="w-2 h-2 rounded-full bg-cyan-500 inline-block shadow-[0_0_5px_rgba(6,182,212,0.6)]" />
                      Ledger Reads
                    </span>
                    <span className="text-[9px] font-mono text-slate-500">
                      {formatBytes(ledger_read_bytes)} / {formatBytes(LIMITS.LEDGER_READ)} limit
                    </span>
                  </div>
                  {/* Budget fill bar */}
                  <div className="h-1.5 w-full bg-slate-950 rounded-full overflow-hidden border border-slate-800 mb-3">
                    <div
                      className="h-full rounded-full transition-all duration-1000"
                      style={{
                        width: `${ioReadPct}%`,
                        background: ioReadPct > 80
                          ? 'linear-gradient(90deg,#f43f5e,#fb923c)'
                          : ioReadPct > 50
                          ? 'linear-gradient(90deg,#eab308,#f59e0b)'
                          : 'linear-gradient(90deg,#0891b2,#06b6d4)',
                      }}
                    />
                  </div>
                  <div className="flex justify-between text-[8px] font-mono text-slate-600 mb-3">
                    <span>{ioReadPct.toFixed(1)}% of 150 KB budget consumed</span>
                    <span>{formatBytes(LIMITS.LEDGER_READ - ledger_read_bytes)} remaining</span>
                  </div>

                  {/* Read sub-segment rows */}
                  <div className="space-y-1.5">
                    {readSegments.map((seg, i) => {
                      const hex = READ_SHADES[i] ?? READ_SHADES[READ_SHADES.length - 1];
                      const isHov = hoveredSegmentId === seg.id;
                      return (
                        <button
                          key={seg.id}
                          onMouseEnter={() => setHoveredSegmentId(seg.id)}
                          onMouseLeave={() => setHoveredSegmentId(null)}
                          className={cn(
                            'w-full flex items-center gap-2.5 rounded-md px-2.5 py-2 text-left',
                            'border border-cyan-900/30 bg-cyan-950/20 transition-all duration-200',
                            isHov ? 'ring-1 ring-cyan-500/30 scale-[1.01] bg-cyan-950/40' : 'hover:bg-cyan-950/30',
                          )}
                        >
                          <span className="w-2 h-2 rounded-sm shrink-0" style={{ backgroundColor: hex }} />
                          <span className="text-[10px] font-mono text-cyan-200 flex-1 truncate">{seg.shortLabel}</span>
                          <div className="w-24 h-1 bg-black/30 rounded-full overflow-hidden shrink-0">
                            <div
                              className="h-full rounded-full"
                              style={{ width: `${seg.share}%`, backgroundColor: hex, opacity: 0.9 }}
                            />
                          </div>
                          <span className="text-[10px] font-mono font-bold text-cyan-300 w-14 text-right shrink-0">
                            {formatBytes(seg.bytes)}
                          </span>
                          <span className="text-[9px] font-mono text-slate-500 w-7 text-right shrink-0">
                            {seg.share}%
                          </span>
                        </button>
                      );
                    })}
                  </div>
                </div>

                {/* WRITES section */}
                <div className="rounded-xl border border-rose-900/40 bg-rose-950/10 p-3">
                  {/* Section header + budget bar */}
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-[10px] font-mono font-bold text-rose-400 uppercase tracking-widest flex items-center gap-1.5">
                      <span className="w-2 h-2 rounded-full bg-rose-500 inline-block shadow-[0_0_5px_rgba(244,63,94,0.6)]" />
                      Ledger Writes
                    </span>
                    <span className="text-[9px] font-mono text-slate-500">
                      {formatBytes(ledger_write_bytes)} / {formatBytes(LIMITS.LEDGER_WRITE)} limit
                    </span>
                  </div>
                  {/* Budget fill bar */}
                  <div className="h-1.5 w-full bg-slate-950 rounded-full overflow-hidden border border-slate-800 mb-3">
                    <div
                      className="h-full rounded-full transition-all duration-1000"
                      style={{
                        width: `${ioWritePct}%`,
                        background: ioWritePct > 80
                          ? 'linear-gradient(90deg,#f43f5e,#fb923c)'
                          : ioWritePct > 50
                          ? 'linear-gradient(90deg,#eab308,#f59e0b)'
                          : 'linear-gradient(90deg,#be123c,#f43f5e)',
                      }}
                    />
                  </div>
                  <div className="flex justify-between text-[8px] font-mono text-slate-600 mb-3">
                    <span>{ioWritePct.toFixed(1)}% of 100 KB budget consumed</span>
                    <span>{formatBytes(LIMITS.LEDGER_WRITE - ledger_write_bytes)} remaining</span>
                  </div>

                  {/* Write sub-segment rows */}
                  <div className="space-y-1.5">
                    {writeSegments.map((seg, i) => {
                      const hex = WRITE_SHADES[i] ?? WRITE_SHADES[WRITE_SHADES.length - 1];
                      const isHov = hoveredSegmentId === seg.id;
                      return (
                        <button
                          key={seg.id}
                          onMouseEnter={() => setHoveredSegmentId(seg.id)}
                          onMouseLeave={() => setHoveredSegmentId(null)}
                          className={cn(
                            'w-full flex items-center gap-2.5 rounded-md px-2.5 py-2 text-left',
                            'border border-rose-900/30 bg-rose-950/20 transition-all duration-200',
                            isHov ? 'ring-1 ring-rose-500/30 scale-[1.01] bg-rose-950/40' : 'hover:bg-rose-950/30',
                          )}
                        >
                          <span className="w-2 h-2 rounded-sm shrink-0" style={{ backgroundColor: hex }} />
                          <span className="text-[10px] font-mono text-rose-200 flex-1 truncate">{seg.shortLabel}</span>
                          <div className="w-24 h-1 bg-black/30 rounded-full overflow-hidden shrink-0">
                            <div
                              className="h-full rounded-full"
                              style={{ width: `${seg.share}%`, backgroundColor: hex, opacity: 0.9 }}
                            />
                          </div>
                          <span className="text-[10px] font-mono font-bold text-rose-300 w-14 text-right shrink-0">
                            {formatBytes(seg.bytes)}
                          </span>
                          <span className="text-[9px] font-mono text-slate-500 w-7 text-right shrink-0">
                            {seg.share}%
                          </span>
                        </button>
                      );
                    })}
                  </div>
                </div>

                {/* TX size */}
                <div className="rounded-xl border border-slate-700/40 bg-slate-900/30 p-3">
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-[10px] font-mono font-bold text-slate-400 uppercase tracking-widest">
                      Transaction Size
                    </span>
                    <span className="text-[9px] font-mono text-slate-500">
                      {formatBytes(transaction_size_bytes)} / {formatBytes(LIMITS.TX_SIZE)} limit
                    </span>
                  </div>
                  <div className="h-1.5 w-full bg-slate-950 rounded-full overflow-hidden border border-slate-800">
                    <div
                      className="h-full rounded-full transition-all duration-1000"
                      style={{
                        width: `${txSizePct}%`,
                        background: txSizePct > 80
                          ? 'linear-gradient(90deg,#f43f5e,#fb923c)'
                          : txSizePct > 50
                          ? 'linear-gradient(90deg,#eab308,#a371f7)'
                          : 'linear-gradient(90deg,#7c3aed,#a371f7)',
                      }}
                    />
                  </div>
                  <div className="flex justify-between text-[8px] font-mono text-slate-600 mt-1">
                    <span>{txSizePct.toFixed(1)}% of 70 KB budget</span>
                    <span className={txStyle.text}>{formatBytes(transaction_size_bytes)}</span>
                  </div>
                </div>
              </div>

              {/* ── Inspector panel ─────────────────────────────────────────── */}
              <div className="lg:w-60 bg-slate-950/40 border border-slate-800/70 rounded-xl p-4 shadow-sm flex flex-col justify-between min-h-[260px]">
                <div>
                  <span className="text-[9px] font-bold text-slate-500 uppercase tracking-widest font-mono">
                    SEGMENT INSPECTOR
                  </span>

                  {hoveredSeg ? (() => {
                    const isRead = hoveredSeg.kind === 'read';
                    const budgetBytes = isRead ? LIMITS.LEDGER_READ : LIMITS.LEDGER_WRITE;
                    const kindTotal   = isRead ? ledger_read_bytes  : ledger_write_bytes;
                    const kindPct     = isRead ? ioReadPct : ioWritePct;
                    const segHex      = isRead ? READ_SHADES[readSegments.findIndex(s => s.id === hoveredSeg.id)] ?? READ_SHADES[0]
                                               : WRITE_SHADES[writeSegments.findIndex(s => s.id === hoveredSeg.id)] ?? WRITE_SHADES[0];
                    return (
                      <div className="mt-3 space-y-3">
                        <div>
                          <span className="text-[8px] text-slate-500 font-mono uppercase block mb-1">ENTRY TYPE</span>
                          <div
                            className="flex items-center gap-2 rounded px-2 py-1.5 border"
                            style={{
                              borderColor: `${segHex}40`,
                              backgroundColor: `${segHex}12`,
                            }}
                          >
                            <span className="w-2 h-2 rounded-sm shrink-0" style={{ backgroundColor: segHex }} />
                            <span className="text-xs font-mono font-bold" style={{ color: segHex }}>
                              {hoveredSeg.label}
                            </span>
                          </div>
                        </div>

                        <div className="flex items-center gap-2">
                          <span className="text-[8px] text-slate-500 font-mono uppercase">KIND</span>
                          <span
                            className="text-[9px] font-mono font-bold rounded px-1.5 py-0.5 border uppercase"
                            style={{
                              color: isRead ? '#06b6d4' : '#f43f5e',
                              borderColor: isRead ? '#0e7490' : '#be123c',
                              backgroundColor: isRead ? '#0c4a6e22' : '#4c0519 22',
                            }}
                          >
                            {isRead ? 'READ' : 'WRITE'}
                          </span>
                        </div>

                        <div className="grid grid-cols-2 gap-2">
                          <div className="bg-slate-900 border border-slate-800 rounded p-2">
                            <span className="text-[8px] font-mono text-slate-500 uppercase block">BYTES</span>
                            <span className="text-sm font-mono font-black text-slate-100 mt-0.5 block">
                              {formatBytes(hoveredSeg.bytes)}
                            </span>
                          </div>
                          <div className="bg-slate-900 border border-slate-800 rounded p-2">
                            <span className="text-[8px] font-mono text-slate-500 uppercase block">OF KIND</span>
                            <span className="text-sm font-mono font-black text-slate-100 mt-0.5 block">
                              {hoveredSeg.share}%
                            </span>
                          </div>

                          {/* Share of kind total bar */}
                          <div className="col-span-2 bg-slate-900 border border-slate-800 rounded p-2">
                            <span className="text-[8px] font-mono text-slate-500 uppercase block mb-1">
                              OF {isRead ? '150 KB READ' : '100 KB WRITE'} BUDGET
                            </span>
                            <div className="h-1.5 w-full bg-black/40 rounded-full overflow-hidden">
                              <div
                                className="h-full rounded-full transition-all duration-700"
                                style={{
                                  width: `${Math.min((hoveredSeg.bytes / budgetBytes) * 100, 100)}%`,
                                  backgroundColor: segHex,
                                }}
                              />
                            </div>
                            <span className="text-[9px] font-mono text-slate-400 mt-0.5 block">
                              {((hoveredSeg.bytes / budgetBytes) * 100).toFixed(3)}%
                            </span>
                          </div>

                          {/* Kind total context */}
                          <div className="col-span-2 bg-slate-900 border border-slate-800 rounded p-2">
                            <span className="text-[8px] font-mono text-slate-500 uppercase block mb-0.5">
                              {isRead ? 'TOTAL READ' : 'TOTAL WRITE'} BUDGET
                            </span>
                            <div className="h-1 w-full bg-black/40 rounded-full overflow-hidden">
                              <div
                                className="h-full rounded-full"
                                style={{ width: `${kindPct}%`, backgroundColor: segHex, opacity: 0.5 }}
                              />
                            </div>
                            <span className="text-[9px] font-mono text-slate-500 mt-0.5 block">
                              {formatBytes(kindTotal)} / {formatBytes(budgetBytes)} ({kindPct.toFixed(1)}%)
                            </span>
                          </div>
                        </div>
                      </div>
                    );
                  })() : (
                    <div className="mt-4">
                      <p className="text-xs text-slate-400 font-bold">Hover a segment for details</p>
                      <p className="text-[11px] text-slate-600 mt-2 leading-relaxed">
                        Segments show how read and write byte budgets are distributed
                        across ledger entry types. Each bar fills proportionally to its budget limit.
                      </p>
                      <div className="mt-4 space-y-1.5">
                        <span className="text-[9px] font-mono text-slate-500 uppercase block">Limits</span>
                        <div className="flex items-center gap-2 text-[9px] font-mono text-slate-500">
                          <span className="w-2 h-2 rounded-sm bg-cyan-500 shrink-0" />
                          <span className="flex-1">Ledger Reads</span>
                          <span className="text-cyan-400 font-bold">{formatBytes(LIMITS.LEDGER_READ)}</span>
                        </div>
                        <div className="flex items-center gap-2 text-[9px] font-mono text-slate-500">
                          <span className="w-2 h-2 rounded-sm bg-rose-500 shrink-0" />
                          <span className="flex-1">Ledger Writes</span>
                          <span className="text-rose-400 font-bold">{formatBytes(LIMITS.LEDGER_WRITE)}</span>
                        </div>
                        <div className="flex items-center gap-2 text-[9px] font-mono text-slate-500">
                          <span className="w-2 h-2 rounded-sm bg-violet-500 shrink-0" />
                          <span className="flex-1">Transaction Size</span>
                          <span className="text-violet-400 font-bold">{formatBytes(LIMITS.TX_SIZE)}</span>
                        </div>
                      </div>
                    </div>
                  )}
                </div>

                <div className="border-t border-slate-900 pt-2 mt-4 text-[9px] font-mono text-slate-600 flex items-center justify-between">
                  <span>I/O: {formatBytes(totalIoBytes)}</span>
                  <span className="flex items-center gap-1">
                    <Info className="h-3 w-3" />
                    {allSegments.length} segments
                  </span>
                </div>
              </div>
            </div>

            {/* ── Touched key tiles (real ledger data when available) ───────── */}
            <div>
              <div className="flex items-center justify-between mb-2">
                <span className="text-[9px] font-mono text-slate-500 uppercase tracking-widest">
                  Touched Ledger Keys — {footprintItems.length} entries
                </span>
                <div className="flex gap-3 text-[9px] font-mono text-slate-600">
                  <span className="flex items-center gap-1">
                    <span className="w-1.5 h-1.5 rounded-full bg-cyan-500 inline-block" /> Read
                  </span>
                  <span className="flex items-center gap-1">
                    <span className="w-1.5 h-1.5 rounded-full bg-rose-500 inline-block" /> Read-Write
                  </span>
                </div>
              </div>
              <div className="bg-slate-950/80 p-3 rounded-xl border border-slate-800/70 shadow-inner flex flex-wrap gap-2 max-h-[120px] overflow-y-auto">
                {footprintItems.map((item, idx) => (
                  <button
                    key={`fp-${idx}`}
                    onMouseEnter={() => setHoveredKey(item.key)}
                    onMouseLeave={() => setHoveredKey(null)}
                    className={cn(
                      'flex items-center gap-1.5 px-2.5 py-1.5 rounded-md border text-left transition-all duration-200',
                      item.isWrite
                        ? 'bg-rose-500/5 hover:bg-rose-500/10 border-rose-500/20 hover:border-rose-500/40'
                        : 'bg-cyan-500/5 hover:bg-cyan-500/10 border-cyan-500/20 hover:border-cyan-500/40',
                      hoveredKey === item.key ? 'ring-1 ring-white/20 scale-105' : '',
                    )}
                  >
                    <span className={cn('w-1.5 h-1.5 rounded-full shrink-0',
                      item.isWrite
                        ? 'bg-rose-500 shadow-[0_0_4px_rgba(244,63,94,0.6)]'
                        : 'bg-cyan-500 shadow-[0_0_4px_rgba(6,182,212,0.6)]',
                    )} />
                    <span className="text-[10px] font-mono font-bold text-slate-300">{formatKey(item.key)}</span>
                    {hoveredKey === item.key && (
                      <span className="text-[9px] font-mono text-slate-500 ml-1">
                        {formatBytes(item.sizeBytes)} · {item.ttl}L
                        {item.ttl < 1000 && <span className="text-amber-500 ml-1 animate-pulse">!</span>}
                      </span>
                    )}
                  </button>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* ── Footer summary bar ──────────────────────────────────────────────── */}
      <div className="mt-6 pt-4 border-t border-slate-800/80 grid grid-cols-2 md:grid-cols-4 gap-4 text-center">
        <div className="bg-slate-950/40 p-2.5 rounded-lg border border-slate-800/40">
          <span className="text-[9px] font-mono text-slate-500 block uppercase">STROOP FEE</span>
          <span className="text-xs font-mono font-bold text-slate-300 mt-1 block">{cost_stroops.toLocaleString()} stroops</span>
        </div>
        <div className="bg-slate-950/40 p-2.5 rounded-lg border border-slate-800/40">
          <span className="text-[9px] font-mono text-slate-500 block uppercase">TX SIZE</span>
          <span className={cn('text-xs font-mono font-bold mt-1 block', txStyle.text)}>
            {formatBytes(transaction_size_bytes)} ({txSizePct.toFixed(1)}%)
          </span>
        </div>
        <div className="bg-slate-950/40 p-2.5 rounded-lg border border-slate-800/40">
          <span className="text-[9px] font-mono text-slate-500 block uppercase">LEDGER READS</span>
          <span className={cn('text-xs font-mono font-bold mt-1 block', readStyle.text)}>
            {formatBytes(ledger_read_bytes)} ({ioReadPct.toFixed(1)}%)
          </span>
        </div>
        <div className="bg-slate-950/40 p-2.5 rounded-lg border border-slate-800/40">
          <span className="text-[9px] font-mono text-slate-500 block uppercase">LEDGER WRITES</span>
          <span className={cn('text-xs font-mono font-bold mt-1 block', writeStyle.text)}>
            {formatBytes(ledger_write_bytes)} ({ioWritePct.toFixed(1)}%)
          </span>
        </div>
      </div>
    </div>
  );
}
