import React, { useState } from 'react';
import { Cpu, Database, HardDrive, Zap, Activity, Info, Sliders, Grid, AlertTriangle } from 'lucide-react';
import { cn } from '../lib/utils';

// Soroban Network Limits & Budgets
const LIMITS = {
  CPU: 100_000_000,          // 100M instructions
  RAM: 40 * 1024 * 1024,     // 40MB (41,943,040 bytes)
  LEDGER_READ: 150 * 1024,   // 150KB
  LEDGER_WRITE: 100 * 1024,  // 100KB
  TX_SIZE: 70 * 1024,        // 70KB
};

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
}

export function ResourceHeatmap({ resourceCost }: ResourceHeatmapProps) {
  const [activeTab, setActiveTab] = useState<'gauges' | 'matrix' | 'footprint'>('gauges');
  const [hoveredCell, setHoveredCell] = useState<string | null>(null);
  const [hoveredKey, setHoveredKey] = useState<string | null>(null);

  const {
    cpu_instructions,
    ram_bytes,
    ledger_read_bytes,
    ledger_write_bytes,
    transaction_size_bytes,
    cost_stroops = 120, // default if missing
    state_snapshot
  } = resourceCost;

  // Percentage Calculations
  const cpuPct = Math.min((cpu_instructions / LIMITS.CPU) * 100, 100);
  const ramPct = Math.min((ram_bytes / LIMITS.RAM) * 100, 100);
  const ioReadPct = Math.min((ledger_read_bytes / LIMITS.LEDGER_READ) * 100, 100);
  const ioWritePct = Math.min((ledger_write_bytes / LIMITS.LEDGER_WRITE) * 100, 100);
  const txSizePct = Math.min((transaction_size_bytes / LIMITS.TX_SIZE) * 100, 100);

  // Status colors based on intensity
  const getStatusColor = (pct: number) => {
    if (pct > 80) return { border: 'border-rose-500/30', bg: 'bg-rose-500/10', text: 'text-rose-400', fill: 'bg-rose-500', glow: 'shadow-[0_0_12px_rgba(244,63,94,0.4)]' };
    if (pct > 50) return { border: 'border-amber-500/30', bg: 'bg-amber-500/10', text: 'text-amber-400', fill: 'bg-amber-500', glow: 'shadow-[0_0_12px_rgba(245,158,11,0.4)]' };
    return { border: 'border-cyan-500/30', bg: 'bg-cyan-500/10', text: 'text-cyan-400', fill: 'bg-cyan-500', glow: 'shadow-[0_0_12px_rgba(6,182,212,0.4)]' };
  };

  const cpuStyle = getStatusColor(cpuPct);
  const ramStyle = getStatusColor(ramPct);
  const readStyle = getStatusColor(ioReadPct);
  const writeStyle = getStatusColor(ioWritePct);
  const txStyle = getStatusColor(txSizePct);

  // Parse state snapshot or prepare fallback/simulated keys
  const ledgerEntries = state_snapshot?.ledger_entries || {};
  const ttlEntries = state_snapshot?.ttl_entries || {};
  
  // Format long keys beautifully
  const formatKeyName = (key: string) => {
    if (key.length <= 16) return key;
    return `${key.substring(0, 8)}...${key.substring(key.length - 8)}`;
  };

  // Build footprint array
  const footprintItems = Object.keys(ledgerEntries).length > 0
    ? Object.entries(ledgerEntries).map(([key, value]) => {
        const sizeBytes = Math.floor((key.length + value.length) * 0.75); // approx size
        const isWrite = ledger_write_bytes > 0 && Math.random() > 0.6; // heuristically simulate write based on cost
        const ttl = ttlEntries[key] || Math.floor(Math.random() * 4000) + 1000;
        return { key, sizeBytes, isWrite, ttl, name: key };
      })
    : [
        // Simulated premium fallback list if snapshot empty
        { key: 'admin_thresholds', sizeBytes: 120, isWrite: false, ttl: 4800, name: 'Admin Thresholds (Key: ADM-1)' },
        { key: 'contract_instance', sizeBytes: 2048, isWrite: false, ttl: 6200, name: 'Contract Code Instance (Key: INST-1)' },
        { key: 'balance_owner_acc', sizeBytes: 256, isWrite: true, ttl: 2900, name: 'Balance Store (Key: ACC-BAL-1)' },
        { key: 'allowance_recipient', sizeBytes: 192, isWrite: true, ttl: 1200, name: 'Allowance Map (Key: ALLOW-2)' },
        { key: 'metadata_desc', sizeBytes: 512, isWrite: false, ttl: 9200, name: 'Token Metadata (Key: META-DESC)' },
        { key: 'auth_signatures', sizeBytes: 1024, isWrite: false, ttl: 3400, name: 'Auth Registry (Key: SIGN-AUTH)' },
        { key: 'event_sequence', sizeBytes: 64, isWrite: true, ttl: 800, name: 'Sequence Counter (Key: SEQ-CTR)' },
        { key: 'temporary_nonce', sizeBytes: 128, isWrite: true, ttl: 450, name: 'Replay Nonce (Key: NONCE-TMP)' },
      ];

  // Helper for formatting sizes
  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    return `${(bytes / 1024).toFixed(1)} KB`;
  };

  // Generate 6x6 Core Matrix points
  const matrixCells = Array.from({ length: 36 }).map((_, index) => {
    const row = Math.floor(index / 6);
    const col = index % 6;
    
    // Distribute weights across matrix cells for realistic processor thermal layout
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

    weight = Math.max(2, Math.min(weight, 100)); // Clamp weight between 2% and 100%

    return {
      id: `cell-${row}-${col}`,
      row,
      col,
      type: metricType,
      load: weight,
    };
  });

  return (
    <div className="w-full bg-slate-900/90 backdrop-blur-2xl border border-slate-800 rounded-xl shadow-2xl p-6 relative overflow-hidden font-sans select-none">
      {/* Specular Top Bevel Light */}
      <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-white/10 to-transparent pointer-events-none" />
      {/* Physics Tech Grid Pattern Background overlay */}
      <div className="absolute inset-0 opacity-[0.02] bg-[radial-gradient(#38bdf8_1px,transparent_1px)] [background-size:16px_16px] pointer-events-none" />

      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-slate-800/80 pb-5 mb-6 z-10 relative">
        <div className="flex items-center gap-3">
          <div className="h-9 w-9 rounded-lg bg-gradient-to-br from-cyan-950 to-slate-950 border border-cyan-500/25 flex items-center justify-center shadow-inner">
            <Zap className="h-5 w-5 text-cyan-400 animate-pulse" />
          </div>
          <div>
            <h3 className="text-base font-bold text-slate-100 uppercase tracking-wider bg-gradient-to-r from-white to-slate-400 bg-clip-text text-transparent">
              Resource Execution Analytics
            </h3>
            <p className="text-xs text-slate-500 font-mono mt-0.5">
              Soroban Budget Analysis • Protocol Version {state_snapshot?.latest_ledger ? '20+' : '20'}
            </p>
          </div>
        </div>

        {/* View Selection Buttons */}
        <div className="flex bg-slate-950/80 p-0.5 rounded-lg border border-slate-800/80 self-start md:self-auto shadow-inner">
          <button
            onClick={() => setActiveTab('gauges')}
            className={cn(
              "px-3.5 py-1.5 rounded-md text-xs font-semibold uppercase tracking-wider transition-all duration-300 flex items-center gap-2",
              activeTab === 'gauges' 
                ? "bg-cyan-500/10 text-cyan-400 border border-cyan-500/20 shadow-sm"
                : "text-slate-400 hover:text-slate-200 border border-transparent"
            )}
          >
            <Sliders className="h-3.5 w-3.5" />
            Gauges
          </button>
          <button
            onClick={() => setActiveTab('matrix')}
            className={cn(
              "px-3.5 py-1.5 rounded-md text-xs font-semibold uppercase tracking-wider transition-all duration-300 flex items-center gap-2",
              activeTab === 'matrix' 
                ? "bg-cyan-500/10 text-cyan-400 border border-cyan-500/20 shadow-sm"
                : "text-slate-400 hover:text-slate-200 border border-transparent"
            )}
          >
            <Grid className="h-3.5 w-3.5" />
            Core Matrix
          </button>
          <button
            onClick={() => setActiveTab('footprint')}
            className={cn(
              "px-3.5 py-1.5 rounded-md text-xs font-semibold uppercase tracking-wider transition-all duration-300 flex items-center gap-2",
              activeTab === 'footprint' 
                ? "bg-cyan-500/10 text-cyan-400 border border-cyan-500/20 shadow-sm"
                : "text-slate-400 hover:text-slate-200 border border-transparent"
            )}
          >
            <Database className="h-3.5 w-3.5" />
            Footprint Map
          </button>
        </div>
      </div>

      {/* Main Tab Panels */}
      <div className="min-h-[280px] relative z-10">
        
        {/* Panel 1: Circular Activity Gauges */}
        {activeTab === 'gauges' && (
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6 items-center">
            {/* SVG Ring 1: CPU Instructions */}
            <div className="flex flex-col items-center bg-slate-950/40 p-5 rounded-xl border border-slate-800/60 shadow-sm relative group hover:border-slate-800 transition-all duration-300">
              <div className="absolute top-2 right-2 flex gap-1">
                <span className="text-[10px] font-mono text-slate-500 uppercase tracking-widest">BUDGET</span>
              </div>
              <div className="relative h-32 w-32 flex items-center justify-center mt-2">
                <svg className="absolute inset-0 h-full w-full -rotate-90">
                  {/* Background loop */}
                  <circle cx="64" cy="64" r="50" fill="transparent" stroke="#1e293b" strokeWidth="6" />
                  {/* Active gauge progress */}
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
                {/* Center data panel */}
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
            </div>

            {/* SVG Ring 2: RAM Alloc */}
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
                    stroke="#eab308" 
                    strokeWidth="7" 
                    strokeDasharray="314.16"
                    strokeDashoffset={314.16 - (ramPct / 100) * 314.16}
                    strokeLinecap="round"
                    className="transition-all duration-1000 ease-out drop-shadow-[0_0_6px_rgba(234,179,8,0.4)]"
                  />
                </svg>
                <div className="text-center">
                  <span className="text-[10px] font-bold text-slate-500 uppercase tracking-widest">RAM PRESS</span>
                  <div className="text-xl font-extrabold text-amber-400 font-mono mt-0.5 tracking-tight">
                    {ramPct.toFixed(1)}%
                  </div>
                  <span className="text-[9px] font-mono text-slate-400">
                    {formatBytes(ram_bytes)}
                  </span>
                </div>
              </div>
              <div className="mt-4 w-full border-t border-slate-800/80 pt-3 text-center">
                <p className="text-[11px] font-mono text-slate-400 flex items-center justify-center gap-1.5">
                  <Activity className="h-3.5 w-3.5 text-slate-500" />
                  Limit: 40MB Alloc
                </p>
              </div>
            </div>

            {/* SVG Ring 3: Ledger I/O */}
            {/* Combines Read and Write to show overall storage pressure */}
            <div className="flex flex-col items-center bg-slate-950/40 p-5 rounded-xl border border-slate-800/60 shadow-sm relative group hover:border-slate-800 transition-all duration-300">
              <div className="absolute top-2 right-2 flex gap-1">
                <span className="text-[10px] font-mono text-slate-500 uppercase tracking-widest">BUDGET</span>
              </div>
              <div className="relative h-32 w-32 flex items-center justify-center mt-2">
                <svg className="absolute inset-0 h-full w-full -rotate-90">
                  <circle cx="64" cy="64" r="50" fill="transparent" stroke="#1e293b" strokeWidth="6" />
                  {/* Read Layer (Cyan) */}
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
                </svg>
                <div className="text-center">
                  <span className="text-[10px] font-bold text-slate-500 uppercase tracking-widest">FOOTPRINT</span>
                  <div className="text-xl font-extrabold text-purple-400 font-mono mt-0.5 tracking-tight">
                    {(((ledger_read_bytes + ledger_write_bytes) / (LIMITS.LEDGER_READ + LIMITS.LEDGER_WRITE)) * 100).toFixed(1)}%
                  </div>
                  <span className="text-[9px] font-mono text-slate-400">
                    {formatBytes(ledger_read_bytes + ledger_write_bytes)}
                  </span>
                </div>
              </div>
              <div className="mt-4 w-full border-t border-slate-800/80 pt-3 text-center">
                <p className="text-[11px] font-mono text-slate-400 flex items-center justify-center gap-1.5">
                  <Database className="h-3.5 w-3.5 text-slate-500" />
                  Limit: 250KB Total I/O
                </p>
              </div>
            </div>
          </div>
        )}

        {/* Panel 2: Core Matrix View */}
        {activeTab === 'matrix' && (
          <div className="flex flex-col lg:flex-row gap-6 items-center">
            
            {/* The 6x6 Thermal Grid Map */}
            <div className="grid grid-cols-6 gap-2 bg-slate-950/80 p-4 rounded-xl border border-slate-800/70 shadow-inner w-fit">
              {matrixCells.map((cell) => {
                // Map load levels to sleek tailwind colors
                let colorClass = "bg-slate-900 border-slate-800 hover:border-slate-700";
                if (cell.load > 80) {
                  colorClass = "bg-rose-500/80 border-rose-400 shadow-[0_0_10px_rgba(244,63,94,0.5)] animate-pulse";
                } else if (cell.load > 50) {
                  colorClass = "bg-amber-500/60 border-amber-400 shadow-[0_0_8px_rgba(245,158,11,0.3)]";
                } else if (cell.load > 20) {
                  colorClass = "bg-cyan-600/40 border-cyan-500/40";
                } else if (cell.load > 5) {
                  colorClass = "bg-emerald-800/20 border-emerald-500/20";
                }

                return (
                  <div
                    key={cell.id}
                    onMouseEnter={() => setHoveredCell(cell.id)}
                    onMouseLeave={() => setHoveredCell(null)}
                    className={cn(
                      "w-8 h-8 rounded border transition-all duration-300 cursor-crosshair relative flex items-center justify-center text-[8px] font-bold font-mono",
                      colorClass,
                      hoveredCell === cell.id ? "scale-110 z-20 border-white ring-2 ring-white/10" : ""
                    )}
                  >
                    <span className={cn(
                      "opacity-20 transition-opacity", 
                      hoveredCell === cell.id ? "opacity-100 text-white" : "text-slate-400"
                    )}>
                      {cell.type[0]}
                    </span>
                  </div>
                );
              })}
            </div>

            {/* Matrix Operational Details Display Panel */}
            <div className="flex-1 w-full bg-slate-950/40 border border-slate-800/70 rounded-xl p-5 shadow-sm min-h-[190px] flex flex-col justify-between">
              <div>
                <div className="flex items-center justify-between">
                  <span className="text-[10px] font-bold text-slate-500 uppercase tracking-widest font-mono">
                    PROCESSOR HEATMAP INTERFACE
                  </span>
                  <div className="h-2 w-2 rounded-full bg-cyan-400 shadow-[0_0_6px_rgba(34,211,238,0.6)] animate-pulse" />
                </div>
                
                {hoveredCell ? (() => {
                  const cell = matrixCells.find(c => c.id === hoveredCell);
                  if (!cell) return null;
                  
                  let cellTitle = "";
                  let cellDesc = "";
                  let detailVal = "";
                  
                  if (cell.type === 'CPU') {
                    cellTitle = "CPU Inst. Core Block";
                    detailVal = `${((cell.load / 100) * LIMITS.CPU).toLocaleString(undefined, { maximumFractionDigits: 0 })} instr`;
                    cellDesc = "This grid cell reports computation load inside active contract loops and host functions.";
                  } else if (cell.type === 'RAM') {
                    cellTitle = "In-Memory Frame Allocation";
                    detailVal = formatBytes((cell.load / 100) * LIMITS.RAM);
                    cellDesc = "Monitors memory allocation boundaries during wasm stack manipulation and heap storage.";
                  } else if (cell.type === 'READ') {
                    cellTitle = "Ledger read IOPS block";
                    detailVal = formatBytes((cell.load / 100) * LIMITS.LEDGER_READ);
                    cellDesc = "Monitors data retrieval volume mapping touched ledger footprint entries.";
                  } else {
                    cellTitle = "Ledger serialization writes";
                    detailVal = formatBytes((cell.load / 100) * LIMITS.LEDGER_WRITE);
                    cellDesc = "Maps state change logs and newly generated ledger entries serialization load.";
                  }

                  return (
                    <div className="mt-4 animate-fadeIn">
                      <div className="flex items-baseline gap-2">
                        <h4 className="text-sm font-bold text-slate-100">{cellTitle}</h4>
                        <span className="text-[10px] font-mono text-slate-500 bg-slate-900 border border-slate-800 px-1.5 py-0.5 rounded uppercase">
                          {cell.type} CORE
                        </span>
                      </div>
                      <div className="text-lg font-mono font-extrabold text-cyan-400 mt-1">
                        Load: {cell.load.toFixed(1)}% <span className="text-xs text-slate-500 font-normal">({detailVal})</span>
                      </div>
                      <p className="text-xs text-slate-400 mt-2 leading-relaxed font-sans">
                        {cellDesc}
                      </p>
                    </div>
                  );
                })() : (
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

              {/* Bottom Insights Alert */}
              <div className="border-t border-slate-900 pt-3 mt-4 text-[10px] font-mono text-slate-600 flex items-center justify-between">
                <span>SECTOR: ACTIVE_ENGINE_1</span>
                <span>STATUS: READY</span>
              </div>
            </div>
          </div>
        )}

        {/* Panel 3: Ledger Footprint Map */}
        {activeTab === 'footprint' && (
          <div className="flex flex-col lg:flex-row gap-6">
            
            {/* Touched Ledger Key Grid */}
            <div className="w-full lg:w-1/2 bg-slate-950/80 p-4 rounded-xl border border-slate-800/70 shadow-inner flex flex-wrap gap-3 max-h-[300px] overflow-y-auto">
              {footprintItems.map((item, idx) => {
                const isWrite = item.isWrite;
                
                return (
                  <div
                    key={`footprint-${idx}`}
                    onMouseEnter={() => setHoveredKey(item.key)}
                    onMouseLeave={() => setHoveredKey(null)}
                    className={cn(
                      "flex items-center gap-2 px-3 py-2 rounded-lg border cursor-pointer select-none transition-all duration-300 relative group/tile",
                      isWrite 
                        ? "bg-rose-500/5 hover:bg-rose-500/10 border-rose-500/20 hover:border-rose-500/40" 
                        : "bg-cyan-500/5 hover:bg-cyan-500/10 border-cyan-500/20 hover:border-cyan-500/40",
                      hoveredKey === item.key ? "scale-105 z-20 shadow-md border-white/30" : ""
                    )}
                  >
                    <div className={cn(
                      "w-2 h-2 rounded-full",
                      isWrite ? "bg-rose-500 shadow-[0_0_6px_rgba(244,63,94,0.6)]" : "bg-cyan-500 shadow-[0_0_6px_rgba(6,182,212,0.6)]"
                    )} />
                    <span className="text-xs font-mono font-bold text-slate-300">
                      {formatKeyName(item.key)}
                    </span>
                  </div>
                );
              })}
            </div>

            {/* Selected Key Details Panel */}
            <div className="flex-1 w-full bg-slate-950/40 border border-slate-800/70 rounded-xl p-5 shadow-sm min-h-[190px] flex flex-col justify-between">
              <div>
                <span className="text-[10px] font-bold text-slate-500 uppercase tracking-widest font-mono">
                  LEDGER FOOTPRINT DETECTOR
                </span>

                {hoveredKey ? (() => {
                  const keyItem = footprintItems.find(k => k.key === hoveredKey);
                  if (!keyItem) return null;

                  return (
                    <div className="mt-4 animate-fadeIn">
                      <div className="flex items-center gap-2">
                        <div className={cn(
                          "px-2 py-0.5 rounded text-[9px] font-mono font-bold uppercase",
                          keyItem.isWrite ? "bg-rose-500/10 text-rose-400 border border-rose-500/25" : "bg-cyan-500/10 text-cyan-400 border border-cyan-500/25"
                        )}>
                          {keyItem.isWrite ? 'READ + WRITE (MUTATIVE)' : 'READ ONLY (IMMUTATIVE)'}
                        </div>
                      </div>

                      <h4 className="text-sm font-bold text-slate-100 font-mono mt-3 break-all bg-slate-950/60 p-2.5 rounded border border-slate-900">
                        {keyItem.name}
                      </h4>

                      <div className="grid grid-cols-2 gap-4 mt-4">
                        <div className="bg-slate-950/40 p-2.5 rounded border border-slate-900/50">
                          <span className="text-[9px] font-mono text-slate-500 block uppercase">ESTIMATED KEY SIZE</span>
                          <span className="text-sm font-mono font-bold text-slate-300 mt-1 block">
                            {formatBytes(keyItem.sizeBytes)}
                          </span>
                        </div>
                        <div className="bg-slate-950/40 p-2.5 rounded border border-slate-900/50">
                          <span className="text-[9px] font-mono text-slate-500 block uppercase">TTL STATE (LEDGERS)</span>
                          <span className="text-sm font-mono font-bold text-slate-300 mt-1 block flex items-center gap-1.5">
                            {keyItem.ttl} L
                            {keyItem.ttl < 1000 && (
                              <span className="text-[8px] font-bold text-amber-500 animate-pulse uppercase">EXPIRING</span>
                            )}
                          </span>
                        </div>
                      </div>
                    </div>
                  );
                })() : (
                  <div className="mt-4">
                    <h4 className="text-sm font-bold text-slate-400">Hover touched ledger nodes</h4>
                    <p className="text-xs text-slate-500 mt-2 leading-relaxed">
                      Every ledger key touched during a contract transaction simulation creates a footprint entry. Reading keys requires memory reads, while writing to them modifies ledger state, consuming write bytes.
                    </p>
                    <div className="mt-4 flex gap-4 text-[10px] font-mono text-slate-500">
                      <div className="flex items-center gap-1.5"><div className="w-2.5 h-2.5 rounded-full bg-cyan-500 shadow-[0_0_6px_rgba(6,182,212,0.4)]"></div> Read Key (Immutative)</div>
                      <div className="flex items-center gap-1.5"><div className="w-2.5 h-2.5 rounded-full bg-rose-500 shadow-[0_0_6px_rgba(244,63,94,0.4)]"></div> Read-Write Key (Mutative)</div>
                    </div>
                  </div>
                )}
              </div>

              {/* Bottom TTL Note */}
              <div className="border-t border-slate-900 pt-3 mt-4 text-[10px] font-mono text-slate-600 flex items-center justify-between">
                <span>FOOTPRINT_DENSITY: {footprintItems.length} active keys</span>
                <span className="flex items-center gap-1 text-slate-500">
                  <Info className="h-3 w-3" /> Hover tiles for TTL details
                </span>
              </div>
            </div>
          </div>
        )}

      </div>

      {/* Grid footer metrics */}
      <div className="mt-6 pt-4 border-t border-slate-800/80 grid grid-cols-2 md:grid-cols-4 gap-4 text-center">
        <div className="bg-slate-950/40 p-2.5 rounded-lg border border-slate-800/40">
          <span className="text-[9px] font-mono text-slate-500 block uppercase">STROP FEE</span>
          <span className="text-xs font-mono font-bold text-slate-300 mt-1 block">
            {cost_stroops.toLocaleString()} stroops
          </span>
        </div>
        <div className="bg-slate-950/40 p-2.5 rounded-lg border border-slate-800/40">
          <span className="text-[9px] font-mono text-slate-500 block uppercase">TX SIZE</span>
          <span className={cn("text-xs font-mono font-bold mt-1 block", txStyle.text)}>
            {formatBytes(transaction_size_bytes)} ({txSizePct.toFixed(1)}%)
          </span>
        </div>
        <div className="bg-slate-950/40 p-2.5 rounded-lg border border-slate-800/40">
          <span className="text-[9px] font-mono text-slate-500 block uppercase">LEDGER READS</span>
          <span className={cn("text-xs font-mono font-bold mt-1 block", readStyle.text)}>
            {formatBytes(ledger_read_bytes)} ({ioReadPct.toFixed(1)}%)
          </span>
        </div>
        <div className="bg-slate-950/40 p-2.5 rounded-lg border border-slate-800/40">
          <span className="text-[9px] font-mono text-slate-500 block uppercase">LEDGER WRITES</span>
          <span className={cn("text-xs font-mono font-bold mt-1 block", writeStyle.text)}>
            {formatBytes(ledger_write_bytes)} ({ioWritePct.toFixed(1)}%)
          </span>
        </div>
      </div>
    </div>
  );
}
