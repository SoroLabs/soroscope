import React from "react";
import { Card, CardHeader, CardTitle, CardContent } from "./ui/card";
import { Cpu, Database, HardDrive, AlertTriangle, Zap, FileSpreadsheet } from "lucide-react";
import { cn } from "../lib/utils";

export interface SorobanLimits {
  cpuInstructions: number; // default: 100_000_000 (100M)
  ramBytes: number;        // default: 40 * 1024 * 1024 (40MB)
  writeBytes: number;      // default: 64 * 1024 (64KB)
  footprintKeys: number;   // default: 100
}

export const DEFAULT_SOROBAN_LIMITS: SorobanLimits = {
  cpuInstructions: 100_000_000,
  ramBytes: 40 * 1024 * 1024,
  writeBytes: 64 * 1024,
  footprintKeys: 100,
};

export function calculateHeadroom(used: number, limit: number): {
  consumedPct: number;
  headroomPct: number;
  txsPerLedger: number;
  isExceeded: boolean;
} {
  if (limit <= 0) {
    return { consumedPct: 100, headroomPct: 0, txsPerLedger: 0, isExceeded: true };
  }
  const consumedPct = Math.round((used / limit) * 1000) / 10;
  const headroomPct = Math.max(0, Math.round((100 - (used / limit) * 100) * 10) / 10);
  const txsPerLedger = used > 0 ? Math.floor(limit / used) : Infinity;
  const isExceeded = used > limit || consumedPct > 100;
  return { consumedPct, headroomPct, txsPerLedger, isExceeded };
}

export const getStatusColor = (value: number) => {
  if (value > 85) return "bg-rose-500 shadow-[0_0_12px_rgba(244,63,94,0.6)] animate-pulse"; // Pulsing red (>85% or exceeded)
  if (value >= 60) return "bg-amber-500 shadow-[0_0_12px_rgba(245,158,11,0.6)]"; // Warning amber (60-85%)
  return "bg-emerald-500 shadow-[0_0_12px_rgba(16,185,129,0.6)]"; // Safe green (<60%)
};

export const getTextColor = (value: number) => {
  if (value > 85)
    return "text-rose-400 drop-shadow-[0_0_8px_rgba(244,63,94,0.5)]";
  if (value >= 60)
    return "text-amber-400 drop-shadow-[0_0_8px_rgba(245,158,11,0.5)]";
  return "text-emerald-400 drop-shadow-[0_0_8px_rgba(16,185,129,0.5)]";
};

interface ResourceMetricProps {
  label: string;
  value: number; // percentage 0-100+
  icon: React.ElementType;
  rawUsed?: number;
  limit?: number;
}

const ResourceMetric = ({
  label,
  value,
  icon: Icon,
  rawUsed,
  limit,
}: ResourceMetricProps) => {
  const isExceeded = value > 100;
  const headroomPct = Math.max(0, Math.round((100 - value) * 10) / 10);
  const txsPerLedger =
    limit && rawUsed && rawUsed > 0
      ? Math.floor(limit / rawUsed)
      : value > 0
      ? Math.floor(100 / value)
      : 999;
  const statusColor = getStatusColor(value);
  const textColor = getTextColor(value);

  return (
    <div className="flex flex-col gap-3 relative group/metric">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2.5">
          <div className="p-1.5 rounded-md bg-slate-800/80 border border-slate-700/50 shadow-inner">
            <Icon className="h-3.5 w-3.5 text-slate-400 group-hover/metric:text-slate-200 transition-colors" />
          </div>
          <div className="flex flex-col">
            <span className="text-xs font-semibold uppercase tracking-wider text-slate-400 group-hover/metric:text-slate-300 transition-colors">
              {label}
            </span>
            <span className="text-[10px] text-slate-500 font-mono">
              Headroom: {headroomPct}% · ~{txsPerLedger} tx/ledger
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {isExceeded && (
            <span className="px-1.5 py-0.5 rounded text-[9px] font-bold bg-rose-500/20 text-rose-400 border border-rose-500/30 uppercase animate-pulse">
              Limit Exceeded
            </span>
          )}
          <span
            className={cn(
              "text-base font-bold font-mono tracking-tight",
              textColor,
            )}
          >
            {value}%
          </span>
        </div>
      </div>

      {/* Progress Bar Container with "Groove" effect */}
      <div className="h-2.5 w-full overflow-hidden rounded-sm bg-slate-950/80 shadow-[inset_0_1px_2px_rgba(0,0,0,0.6)] border-b border-white/5">
        <div
          className={cn(
            "h-full transition-all duration-500 ease-out relative",
            statusColor,
          )}
          style={{ width: `${Math.min(value, 100)}%` }}
        >
          {/* Scanline effect on bar */}
          <div className="absolute inset-0 bg-[linear-gradient(90deg,transparent,rgba(255,255,255,0.2),transparent)]" />
        </div>
      </div>
    </div>
  );
};

export interface ResourceLabelProps {
  cpuUsage: number;
  ramUsage: number;
  footprintUsage: number;
  writeBytesUsage?: number;
  limits?: Partial<SorobanLimits>;
  rawUsage?: {
    cpu?: number;
    ram?: number;
    footprint?: number;
    writeBytes?: number;
  };
}

export function ResourceLabel({
  cpuUsage,
  ramUsage,
  footprintUsage,
  writeBytesUsage,
  limits,
  rawUsage,
}: ResourceLabelProps) {
  const effectiveLimits = { ...DEFAULT_SOROBAN_LIMITS, ...limits };
  const isAnyExceeded =
    cpuUsage > 100 ||
    ramUsage > 100 ||
    footprintUsage > 100 ||
    (writeBytesUsage !== undefined && writeBytesUsage > 100) ||
    (rawUsage?.cpu !== undefined && rawUsage.cpu > effectiveLimits.cpuInstructions) ||
    (rawUsage?.ram !== undefined && rawUsage.ram > effectiveLimits.ramBytes) ||
    (rawUsage?.footprint !== undefined && rawUsage.footprint > effectiveLimits.footprintKeys) ||
    (rawUsage?.writeBytes !== undefined && rawUsage.writeBytes > effectiveLimits.writeBytes);

  const hasCritical =
    cpuUsage > 85 ||
    ramUsage > 85 ||
    footprintUsage > 85 ||
    (writeBytesUsage ?? 0) > 85 ||
    isAnyExceeded;

  return (
    <Card className="w-full max-w-md bg-slate-900/80 backdrop-blur-xl border border-white/10 shadow-2xl relative overflow-hidden group">
      {/* 1. Noise Texture Overlay */}
      <div className="absolute inset-0 opacity-[0.03] pointer-events-none mix-blend-overlay bg-[url('https://grainy-gradients.vercel.app/noise.svg')]" />

      {/* 2. Inner Ring for Bevel effect */}
      <div className="absolute inset-[1px] rounded-lg border border-white/5 pointer-events-none" />

      {/* 3. Top Specular Highlight */}
      <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-white/20 to-transparent opacity-50" />

      {/* 4. Technical Corner Accents ("Bolts") */}
      <div className="absolute top-3 left-3 w-1.5 h-1.5 rounded-full bg-slate-700/50 shadow-[inset_0_0_2px_rgba(0,0,0,0.8)] border border-slate-600/30" />
      <div className="absolute top-3 right-3 w-1.5 h-1.5 rounded-full bg-slate-700/50 shadow-[inset_0_0_2px_rgba(0,0,0,0.8)] border border-slate-600/30" />
      <div className="absolute bottom-3 left-3 w-1.5 h-1.5 rounded-full bg-slate-700/50 shadow-[inset_0_0_2px_rgba(0,0,0,0.8)] border border-slate-600/30" />
      <div className="absolute bottom-3 right-3 w-1.5 h-1.5 rounded-full bg-slate-700/50 shadow-[inset_0_0_2px_rgba(0,0,0,0.8)] border border-slate-600/30" />

      <CardHeader className="pb-6 pt-7 relative z-10">
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg font-bold text-slate-100 tracking-tight flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-slate-800 to-slate-900 shadow-inner border border-white/5">
              <Zap className="h-4 w-4 text-cyan-400" />
            </div>
            <span className="bg-gradient-to-br from-white to-slate-400 bg-clip-text text-transparent drop-shadow-sm">
              SYSTEM STATUS
            </span>
          </CardTitle>
          {isAnyExceeded ? (
            <div className="flex items-center gap-1.5 rounded bg-rose-600/90 px-3 py-1 text-[10px] font-bold tracking-widest text-white border border-rose-400 shadow-[0_0_15px_rgba(244,63,94,0.4)] animate-pulse uppercase">
              <AlertTriangle className="h-3 w-3 mb-0.5" />
              Exceeds Network Limit
            </div>
          ) : hasCritical ? (
            <div className="flex items-center gap-1.5 rounded bg-rose-500/10 px-3 py-1 text-[10px] font-bold tracking-widest text-rose-400 border border-rose-500/20 shadow-[0_0_15px_rgba(244,63,94,0.15)] animate-pulse uppercase">
              <AlertTriangle className="h-3 w-3 mb-0.5" />
              Critical
            </div>
          ) : (
            <div className="flex items-center gap-1.5 px-3 py-1">
              <div className="h-1.5 w-1.5 rounded-full bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.8)] animate-pulse" />
              <span className="text-[10px] font-bold tracking-widest text-emerald-500/80 uppercase">
                Active
              </span>
            </div>
          )}
        </div>
      </CardHeader>

      <CardContent className="grid gap-7 relative z-10">
        <ResourceMetric
          label="CPU Load"
          value={cpuUsage}
          icon={Cpu}
          rawUsed={rawUsage?.cpu}
          limit={effectiveLimits.cpuInstructions}
        />
        <ResourceMetric
          label="Ledger Size"
          value={footprintUsage}
          icon={Database}
          rawUsed={rawUsage?.footprint}
          limit={effectiveLimits.footprintKeys}
        />
        <ResourceMetric
          label="RAM Alloc"
          value={ramUsage}
          icon={HardDrive}
          rawUsed={rawUsage?.ram}
          limit={effectiveLimits.ramBytes}
        />
        {writeBytesUsage !== undefined && (
          <ResourceMetric
            label="Write Bytes"
            value={writeBytesUsage}
            icon={FileSpreadsheet}
            rawUsed={rawUsage?.writeBytes}
            limit={effectiveLimits.writeBytes}
          />
        )}

        {/* Footer Technical Detail */}
        <div className="mt-2 flex justify-between items-center border-t border-white/5 pt-4">
          <span className="text-[10px] font-mono text-slate-600">
            ID: CN-2948-X
          </span>
          <span className="text-[10px] font-mono text-slate-600">
            REFRESH: 500ms
          </span>
        </div>
      </CardContent>
    </Card>
  );
}
