// Issue #814: Canvas heatmap renderer & matrix visualization
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Cpu, Database, HardDrive, Zap, Activity, Info, Sliders, Grid, AlertTriangle } from 'lucide-react';
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

// Matrix canvas constants
const CELL_SIZE = 32;
const GAP = 8;
const PADDING = 16;
const GRID_COLS = 6;
const GRID_ROWS = 6;
const NATURAL_WIDTH = GRID_COLS * CELL_SIZE + (GRID_COLS - 1) * GAP + PADDING * 2;
const NATURAL_HEIGHT = GRID_ROWS * CELL_SIZE + (GRID_ROWS - 1) * GAP + PADDING * 2;

interface Cell {
  id: string;
  row: number;
  col: number;
  type: 'CPU' | 'RAM' | 'READ' | 'WRITE';
  load: number;
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

function getCellColors(load: number): { fill: string; stroke: string } {
  if (load > 80) return { fill: 'rgba(244,63,94,0.8)', stroke: 'rgba(244,63,94,1)' };
  if (load > 50) return { fill: 'rgba(245,158,11,0.6)', stroke: 'rgba(245,158,11,1)' };
  if (load > 20) return { fill: 'rgba(8,145,178,0.4)', stroke: 'rgba(6,182,212,0.4)' };
  if (load > 5) return { fill: 'rgba(6,78,59,0.2)', stroke: 'rgba(16,185,129,0.2)' };
  return { fill: 'rgba(30,41,59,1)', stroke: 'rgba(51,65,85,1)' };
}

export function ResourceHeatmap({ resourceCost }: ResourceHeatmapProps) {
  const [activeTab, setActiveTab] = useState<'gauges' | 'matrix' | 'footprint'>('gauges');
  const [hoveredCell, setHoveredCell] = useState<string | null>(null);
  const [hoveredKey, setHoveredKey] = useState<string | null>(null);
  const [tooltip, setTooltip] = useState<{ x: number; y: number; cell: Cell } | null>(null);

  const {
    cpu_instructions,
    ram_bytes,
    ledger_read_bytes,
    ledger_write_bytes,
    transaction_size_bytes,
    cost_stroops = 120,
    state_snapshot
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
    ? Object.entries(ledgerEntries).map(([key, value]) => {
        const sizeBytes = Math.floor((key.length + value.length) * 0.75);
        const isWrite = ledger_write_bytes > 0 && Math.random() > 0.6;
        const ttl = ttlEntries[key] || Math.floor(Math.random() * 4000) + 1000;
        return { key, sizeBytes, isWrite, ttl, name: key };
      })
    : [
        { key: 'admin_thresholds', sizeBytes: 120, isWrite: false, ttl: 4800, name: 'Admin Thresholds (Key: ADM-1)' },
        { key: 'contract_instance', sizeBytes: 2048, isWrite: false, ttl: 6200, name: 'Contract Code Instance (Key: INST-1)' },
        { key: 'balance_owner_acc', sizeBytes: 256, isWrite: true, ttl: 2900, name: 'Balance Store (Key: ACC-BAL-1)' },
        { key: 'allowance_recipient', sizeBytes: 192, isWrite: true, ttl: 1200, name: 'Allowance Map (Key: ALLOW-2)' },
        { key: 'metadata_desc', sizeBytes: 512, isWrite: false, ttl: 9200, name: 'Token Metadata (Key: META-DESC)' },
        { key: 'auth_signatures', sizeBytes: 1024, isWrite: false, ttl: 3400, name: 'Auth Registry (Key: SIGN-AUTH)' },
        { key: 'event_sequence', sizeBytes: 64, isWrite: true, ttl: 800, name: 'Sequence Counter (Key: SEQ-CTR)' },
        { key: 'temporary_nonce', sizeBytes: 128, isWrite: true, ttl: 450, name: 'Replay Nonce (Key: NONCE-TMP)' },
      ];

  const formatKey = (key: string) =>
    key.length <= 16 ? key : `${key.slice(0, 8)}…${key.slice(-8)}`;

  // Generate 6x6 Core Matrix points (memoized for canvas performance)
  const matrixCells = useMemo(() => Array.from({ length: 36 }).map((_, index) => {
    const row = Math.floor(index / 6);
    const col = index % 6;
    
    let metricType: 'CPU' | 'RAM' | 'READ' | 'WRITE';
    let weight = 0;
    
    if (row < 2) {
      metricType = 'CPU';
      weight = cpuPct * (0.4 + Math.sin(index + 1) * 0.3);
    } else if (row < 4) {
      metricType = 'RAM';
      weight = ramPct * (0.5 + Math.cos(index) * 0.25);
    } else if (col < 3) {
      metricType = 'READ';
      weight = ioReadPct * (0.6 + Math.sin(col) * 0.2);
    } else {
      metricType = 'WRITE';
      weight = ioWritePct * (0.4 + Math.cos(row) * 0.3);
    }

    weight = Math.max(2, Math.min(weight, 100));

    return {
      id: `cell-${row}-${col}`,
      row,
      col,
      type: metricType,
      load: weight,
    };
  }), [cpuPct, ramPct, ioReadPct, ioWritePct]);

  // Canvas refs and transform state for matrix zoom/pan
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const transformRef = useRef({ scale: 1, offsetX: 0, offsetY: 0 });
  const isPanning = useRef(false);
  const panStart = useRef({ x: 0, y: 0 });
  const animationFrameRef = useRef<number | null>(null);
  const hoveredCellRef = useRef<string | null>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const touchGestureRef = useRef<{
    startX: number;
    startY: number;
    lastX: number;
    lastY: number;
    moved: boolean;
    pinched: boolean;
    startDistance: number;
    startScale: number;
    startOffsetX: number;
    startOffsetY: number;
    startCenterX: number;
    startCenterY: number;
  } | null>(null);

  const requestRedraw = useCallback(() => {
    if (animationFrameRef.current !== null) {
      cancelAnimationFrame(animationFrameRef.current);
    }
    animationFrameRef.current = requestAnimationFrame(() => {
      drawMatrix();
      animationFrameRef.current = null;
    });
  }, []);

  const drawMatrix = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const { scale, offsetX, offsetY } = transformRef.current;

    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.save();
    ctx.translate(offsetX, offsetY);
    ctx.scale(scale, scale);

    const totalCellSize = CELL_SIZE + GAP;

    for (const cell of matrixCells) {
      const x = PADDING + cell.col * totalCellSize;
      const y = PADDING + cell.row * totalCellSize;
      const colors = getCellColors(cell.load);

      // Cell fill
      ctx.fillStyle = colors.fill;
      ctx.fillRect(x, y, CELL_SIZE, CELL_SIZE);

      // Cell border
      ctx.strokeStyle = colors.stroke;
      ctx.lineWidth = 1 / scale;
      ctx.strokeRect(x, y, CELL_SIZE, CELL_SIZE);

      // Type letter
      ctx.fillStyle = 'rgba(148,163,184,0.2)';
      ctx.font = `${Math.max(8, 12 / scale)}px monospace`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(cell.type[0], x + CELL_SIZE / 2, y + CELL_SIZE / 2);

      // Hover highlight
      if (hoveredCellRef.current === cell.id) {
        ctx.strokeStyle = 'rgba(255,255,255,0.9)';
        ctx.lineWidth = 2 / scale;
        ctx.strokeRect(x - 1, y - 1, CELL_SIZE + 2, CELL_SIZE + 2);
      }
    }

    ctx.restore();
  }, [matrixCells]);

  // Initial draw and resize handling
  useEffect(() => {
    drawMatrix();
  }, [drawMatrix]);

  useEffect(() => {
    const handleResize = () => requestRedraw();
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [requestRedraw]);

  const handleWheel = useCallback((e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
    const { scale, offsetX, offsetY } = transformRef.current;
    const newScale = Math.min(Math.max(scale * zoomFactor, 0.1), 10);

    const newOffsetX = mouseX - (mouseX - offsetX) * (newScale / scale);
    const newOffsetY = mouseY - (mouseY - offsetY) * (newScale / scale);

    transformRef.current = { scale: newScale, offsetX: newOffsetX, offsetY: newOffsetY };
    requestRedraw();
  }, [requestRedraw]);

  const handleMouseDown = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    isPanning.current = true;
    panStart.current = { x: e.clientX - transformRef.current.offsetX, y: e.clientY - transformRef.current.offsetY };
    if (canvasRef.current) {
      canvasRef.current.style.cursor = 'grabbing';
    }
  }, []);

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    if (isPanning.current) {
      transformRef.current.offsetX = e.clientX - panStart.current.x;
      transformRef.current.offsetY = e.clientY - panStart.current.y;
      requestRedraw();
      return;
    }

    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const { scale, offsetX, offsetY } = transformRef.current;
    const canvasX = (mouseX - offsetX) / scale;
    const canvasY = (mouseY - offsetY) / scale;

    const totalCellSize = CELL_SIZE + GAP;
    let found: Cell | null = null;
    for (const cell of matrixCells) {
      const x = PADDING + cell.col * totalCellSize;
      const y = PADDING + cell.row * totalCellSize;
      if (canvasX >= x && canvasX < x + CELL_SIZE && canvasY >= y && canvasY < y + CELL_SIZE) {
        found = cell;
        break;
      }
    }

    const newHoveredId = found ? found.id : null;
    if (hoveredCellRef.current !== newHoveredId) {
      hoveredCellRef.current = newHoveredId;
      setHoveredCell(newHoveredId);
      if (found) {
        setTooltip({ x: e.clientX, y: e.clientY, cell: found });
      } else {
        setTooltip(null);
      }
      requestRedraw();
    } else if (found && tooltip) {
      setTooltip({ ...tooltip, x: e.clientX, y: e.clientY });
    }
  }, [matrixCells, requestRedraw, tooltip]);

  const handleMouseUp = useCallback(() => {
    isPanning.current = false;
    if (canvasRef.current) {
      canvasRef.current.style.cursor = 'crosshair';
    }
  }, []);

  const handleMouseLeave = useCallback(() => {
    isPanning.current = false;
    hoveredCellRef.current = null;
    setHoveredCell(null);
    setTooltip(null);
    if (canvasRef.current) {
      canvasRef.current.style.cursor = 'crosshair';
    }
    requestRedraw();
  }, [requestRedraw]);

  const findCellAt = useCallback((clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    const { scale, offsetX, offsetY } = transformRef.current;
    const canvasX = (clientX - rect.left - offsetX) / scale;
    const canvasY = (clientY - rect.top - offsetY) / scale;
    const totalCellSize = CELL_SIZE + GAP;
    return matrixCells.find((cell) => {
      const x = PADDING + cell.col * totalCellSize;
      const y = PADDING + cell.row * totalCellSize;
      return canvasX >= x && canvasX < x + CELL_SIZE && canvasY >= y && canvasY < y + CELL_SIZE;
    }) ?? null;
  }, [matrixCells]);

  const handleTouchStart = useCallback((e: React.TouchEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const touches = Array.from(e.touches);
    if (touches.length === 0) return;
    const first = touches[0];
    const second = touches[1];
    const centerX = second ? (first.clientX + second.clientX) / 2 : first.clientX;
    const centerY = second ? (first.clientY + second.clientY) / 2 : first.clientY;
    const distance = second ? Math.hypot(first.clientX - second.clientX, first.clientY - second.clientY) : 0;
    const transform = transformRef.current;
    touchGestureRef.current = {
      startX: first.clientX,
      startY: first.clientY,
      lastX: first.clientX,
      lastY: first.clientY,
      moved: false,
      pinched: touches.length > 1,
      startDistance: distance,
      startScale: transform.scale,
      startOffsetX: transform.offsetX,
      startOffsetY: transform.offsetY,
      startCenterX: centerX,
      startCenterY: centerY,
    };
    isPanning.current = false;
    setTooltip(null);
  }, []);

  const handleTouchMove = useCallback((e: React.TouchEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const gesture = touchGestureRef.current;
    const touches = Array.from(e.touches);
    if (!gesture || touches.length === 0) return;
    const first = touches[0];
    const second = touches[1];
    if (Math.hypot(first.clientX - gesture.startX, first.clientY - gesture.startY) > 8) {
      gesture.moved = true;
    }

    if (second) {
      gesture.pinched = true;
      const distance = Math.hypot(first.clientX - second.clientX, first.clientY - second.clientY);
      const centerX = (first.clientX + second.clientX) / 2;
      const centerY = (first.clientY + second.clientY) / 2;
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const startLocalX = gesture.startCenterX - rect.left;
      const startLocalY = gesture.startCenterY - rect.top;
      const currentLocalX = centerX - rect.left;
      const currentLocalY = centerY - rect.top;
      const scale = Math.min(10, Math.max(0.5, gesture.startScale * distance / Math.max(gesture.startDistance, 1)));
      const ratio = scale / gesture.startScale;
      transformRef.current = {
        scale,
        offsetX: currentLocalX - (startLocalX - gesture.startOffsetX) * ratio,
        offsetY: currentLocalY - (startLocalY - gesture.startOffsetY) * ratio,
      };
      requestRedraw();
      return;
    }

    if (!gesture.pinched) {
      transformRef.current.offsetX += first.clientX - gesture.lastX;
      transformRef.current.offsetY += first.clientY - gesture.lastY;
      requestRedraw();
    }
    gesture.lastX = first.clientX;
    gesture.lastY = first.clientY;
  }, [requestRedraw]);

  const handleTouchEnd = useCallback((e: React.TouchEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const gesture = touchGestureRef.current;
    if (e.touches.length === 0) {
      if (gesture && !gesture.moved && !gesture.pinched && e.changedTouches.length > 0) {
        const touch = e.changedTouches[0];
        const cell = findCellAt(touch.clientX, touch.clientY);
        if (cell) {
          hoveredCellRef.current = cell.id;
          setHoveredCell(cell.id);
          setTooltip({ x: touch.clientX, y: touch.clientY, cell });
          requestRedraw();
        } else {
          hoveredCellRef.current = null;
          setHoveredCell(null);
          setTooltip(null);
        }
      }
      touchGestureRef.current = null;
    } else if (e.touches.length === 1 && gesture) {
      // A finger lifted from a pinch should not become a fresh pan or tap.
      gesture.lastX = e.touches[0].clientX;
      gesture.lastY = e.touches[0].clientY;
    }
  }, [findCellAt, requestRedraw]);

  const handleTouchCancel = useCallback(() => {
    touchGestureRef.current = null;
  }, []);

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
            {/* SVG Ring 1: CPU Instructions */}
            <div className="flex flex-col items-center bg-slate-950/40 p-5 rounded-xl border border-slate-800/60 shadow-sm relative group hover:border-slate-800 transition-all duration-300">
              <div className="absolute top-2 right-2 flex gap-1">
                <span className="text-[10px] font-mono text-slate-500 uppercase tracking-widest">BUDGET</span>
              </div>
              <div className="relative h-32 w-32 flex items-center justify-center mt-2">
                <svg className="absolute inset-0 h-full w-full -rotate-90">
                  <circle cx="64" cy="64" r="50" fill="transparent" stroke="#1e293b" strokeWidth="6" />
                  <circle 
                    cx="64" 
                    cy="64" 
                    r="50" 
                    fill="transparent" 
                    stroke="#06b6d4" 
                    strokeWidth="7" 
                    strokeDasharray="314.16"
                    strokeDashoffset={314.16 - (cpuPct / 100) * 314.16}
                    strokeLinecap="round"
                    className="transition-all duration-1000 ease-out drop-shadow-[0_0_6px_rgba(6,182,212,0.4)]"
                  />
                </svg>
                <div className="text-center">
                  <span className="text-[10px] font-bold text-slate-500 uppercase tracking-widest">CPU LOAD</span>
                  <div className="text-xl font-extrabold text-cyan-400 font-mono mt-0.5 tracking-tight">
                    {cpuPct.toFixed(1)}%
                  </div>
                  <span className="text-[9px] font-mono text-slate-400">
                    {new Intl.NumberFormat('en-US', { notation: 'compact' }).format(cpu_instructions)} ops
                  </span>
                </div>
              </div>
              <div className="mt-4 w-full border-t border-slate-800/80 pt-3 text-center">
                <p className="text-[11px] font-mono text-slate-400 flex items-center justify-center gap-1.5">
                  <Cpu className="h-3.5 w-3.5 text-slate-500" />
                  Limit: 100M instructions
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

            {/* SVG Ring 3: Ledger I/O */}
            <div className="flex flex-col items-center bg-slate-950/40 p-5 rounded-xl border border-slate-800/60 shadow-sm relative group hover:border-slate-800 transition-all duration-300">
              <div className="absolute top-2 right-2 flex gap-1">
                <span className="text-[10px] font-mono text-slate-500 uppercase tracking-widest">BUDGET</span>
              </div>
              <div className="relative h-32 w-32 flex items-center justify-center mt-2">
                <svg className="absolute inset-0 h-full w-full -rotate-90">
                  <circle cx="64" cy="64" r="50" fill="transparent" stroke="#1e293b" strokeWidth="6" />
                  <circle 
                    cx="64" 
                    cy="64" 
                    r="50" 
                    fill="transparent" 
                    stroke="#a371f7" 
                    strokeWidth="7" 
                    strokeDasharray="314.16"
                    strokeDashoffset={314.16 - (((ledger_read_bytes + ledger_write_bytes) / (LIMITS.LEDGER_READ + LIMITS.LEDGER_WRITE)) * 100) * 3.1416}
                    strokeLinecap="round"
                    className="transition-all duration-1000 ease-out drop-shadow-[0_0_6px_rgba(163,113,247,0.4)]"
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

        {/* Panel 2: Core Matrix View */}
        {activeTab === 'matrix' && (
          <div className="flex flex-col lg:flex-row gap-6 items-center">
            
            {/* Canvas-based 6x6 Thermal Grid Map */}
            <div className="relative w-fit">
              <canvas
                ref={canvasRef}
                width={NATURAL_WIDTH}
                height={NATURAL_HEIGHT}
                className="rounded-xl border border-slate-800/70 shadow-inner cursor-crosshair"
                style={{ width: NATURAL_WIDTH, height: NATURAL_HEIGHT, touchAction: 'none' }}
                onWheel={handleWheel}
                onMouseDown={handleMouseDown}
                onMouseMove={handleMouseMove}
                onMouseUp={handleMouseUp}
                onMouseLeave={handleMouseLeave}
                onTouchStart={handleTouchStart}
                onTouchMove={handleTouchMove}
                onTouchEnd={handleTouchEnd}
                onTouchCancel={handleTouchCancel}
              />
              {tooltip && (
                <div
                  ref={tooltipRef}
                  className="fixed z-50 pointer-events-none bg-slate-950/95 border border-slate-800 rounded-lg px-3 py-2 text-xs shadow-xl backdrop-blur-xl"
                  style={{ left: tooltip.x + 12, top: tooltip.y + 12 }}
                >
                  <div className="font-bold text-slate-100 mb-1">{tooltip.cell.type} Core</div>
                  <div className="text-slate-400">Load: {tooltip.cell.load.toFixed(1)}%</div>
                  <div className="text-slate-500 text-[10px] mt-1">
                    {tooltip.cell.type === 'CPU' && `${((tooltip.cell.load / 100) * LIMITS.CPU).toLocaleString(undefined, { maximumFractionDigits: 0 })} instr`}
                    {tooltip.cell.type === 'RAM' && formatBytes((tooltip.cell.load / 100) * LIMITS.RAM)}
                    {tooltip.cell.type === 'READ' && formatBytes((tooltip.cell.load / 100) * LIMITS.LEDGER_READ)}
                    {tooltip.cell.type === 'WRITE' && formatBytes((tooltip.cell.load / 100) * LIMITS.LEDGER_WRITE)}
                  </div>
                </div>
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
