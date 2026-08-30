import React from "react";
import { Card, CardHeader, CardTitle, CardContent } from "./ui/card";
import { Cpu, Database, HardDrive, AlertTriangle, Zap } from "lucide-react";
import { cn } from "../lib/utils";

interface ResourceMetricProps {
  label: string;
  value: number; // percentage 0-100
  icon: React.ElementType;
}

const getStatusColor = (value: number) => {
  if (value > 85) return "bg-rose-500 shadow-[0_0_12px_rgba(244,63,94,0.6)]"; // Critical
  if (value >= 60) return "bg-amber-500 shadow-[0_0_12px_rgba(245,158,11,0.6)]"; // Warning
  return "bg-emerald-500 shadow-[0_0_12px_rgba(16,185,129,0.6)]"; // Safe
};

const getTextColor = (value: number) => {
  if (value > 85)
    return "text-rose-400 drop-shadow-[0_0_8px_rgba(244,63,94,0.5)]";
  if (value >= 60)
    return "text-amber-400 drop-shadow-[0_0_8px_rgba(245,158,11,0.5)]";
  return "text-emerald-400 drop-shadow-[0_0_8px_rgba(16,185,129,0.5)]";
};

const ResourceMetric = ({ label, value, icon: Icon }: ResourceMetricProps) => {
  const statusColor = getStatusColor(value);
  const textColor = getTextColor(value);

  return (
    <div className="flex flex-col gap-3 relative group/metric">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2.5">
          <div className="p-1.5 rounded-md bg-slate-800/80 border border-slate-700/50 shadow-inner">
            <Icon className="h-3.5 w-3.5 text-slate-400 group-hover/metric:text-slate-200 transition-colors" />
          </div>
          <span className="text-xs font-semibold uppercase tracking-wider text-slate-400 group-hover/metric:text-slate-300 transition-colors">
            {label}
          </span>
        </div>
        <span
          className={cn(
            "text-base font-bold font-mono tracking-tight",
            textColor,
          )}
        >
          {value}%
        </span>
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

interface ResourceLabelProps {
  cpuUsage: number;
  ramUsage: number;
  footprintUsage: number;
}

export function ResourceLabel({
  cpuUsage,
  ramUsage,
  footprintUsage,
}: ResourceLabelProps) {
  const hasCritical = cpuUsage > 85 || ramUsage > 85 || footprintUsage > 85;

  return (
    <Card className="w-full max-w-md bg-slate-900/80 backdrop-blur-xl border border-white/10 shadow-2xl relative overflow-hidden group">
      {/* Tempered Glass Physical Attributes */}

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
          {hasCritical ? (
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
        <ResourceMetric label="CPU Load" value={cpuUsage} icon={Cpu} />
        <ResourceMetric
          label="Ledger Size"
          value={footprintUsage}
          icon={Database}
        />
        <ResourceMetric label="RAM Alloc" value={ramUsage} icon={HardDrive} />

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
