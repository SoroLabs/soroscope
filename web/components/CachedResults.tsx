'use client';

import React, { useEffect, useState, useCallback } from 'react';
import { Database, RefreshCw, Trash2, WifiOff, FileCode, Clock } from 'lucide-react';
import {
  getCachedAnalyses,
  deleteCachedAnalysis,
  type CachedAnalysis,
} from '../lib/offlineStorage';
import type { InvocationResult } from '../lib/sorobantypes';

interface CachedResultsProps {
  onSelectResult: (result: InvocationResult) => void;
}

/**
 * CachedResults — shows analyses available offline from IndexedDB.
 *
 * Appears in the history tab so users can review past results without a
 * network connection. Each entry shows the function name, timestamp, and
 * a delete button.
 */
export function CachedResults({ onSelectResult }: CachedResultsProps) {
  const [cached, setCached] = useState<CachedAnalysis[]>([]);
  const [loading, setLoading] = useState(true);
  const [isOnline, setIsOnline] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const results = await getCachedAnalyses();
      setCached(results);
    } catch {
      setCached([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    setIsOnline(navigator.onLine);

    const handleOnline = () => setIsOnline(true);
    const handleOffline = () => setIsOnline(false);
    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);

    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, [refresh]);

  const handleDelete = useCallback(async (id: string) => {
    await deleteCachedAnalysis(id);
    setCached((prev) => prev.filter((item) => item.id !== id));
  }, []);

  if (loading) {
    return (
      <div className="rounded-xl border border-slate-800 bg-slate-900/80 p-4">
        <div className="flex items-center gap-2 text-sm text-slate-400">
          <RefreshCw className="h-4 w-4 animate-spin" />
          Loading cached results…
        </div>
      </div>
    );
  }

  if (cached.length === 0) {
    return (
      <div className="rounded-xl border border-slate-800 bg-slate-900/80 p-6 text-center">
        <Database className="h-8 w-8 mx-auto mb-2 text-slate-600" />
        <p className="text-sm text-slate-400 font-medium">No cached results</p>
        <p className="text-xs text-slate-500 mt-1">
          Run an analysis while online to cache it for offline viewing.
        </p>
      </div>
    );
  }

  return (
    <div className="rounded-xl border border-slate-800 bg-slate-900/80 p-4">
      {/* Header */}
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-cyan-500/10 border border-cyan-500/20">
            <Database className="h-4 w-4 text-cyan-400" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-slate-100">
              Cached Results
            </h3>
            <p className="text-[11px] text-slate-500">
              {cached.length} {cached.length === 1 ? 'analysis' : 'analyses'} available offline
            </p>
          </div>
        </div>

        {/* Offline indicator badge */}
        {!isOnline && (
          <span className="flex items-center gap-1.5 rounded-full bg-amber-500/10 border border-amber-500/20 px-2.5 py-1 text-[10px] font-semibold text-amber-400">
            <WifiOff className="h-3 w-3" />
            Offline
          </span>
        )}

        <button
          type="button"
          onClick={refresh}
          aria-label="Refresh cached results"
          className="ml-auto flex h-7 w-7 items-center justify-center rounded-lg text-slate-400 transition hover:bg-slate-800 hover:text-slate-200"
        >
          <RefreshCw className="h-3.5 w-3.5" />
        </button>
      </div>

      {/* Cached analysis list */}
      <div className="space-y-2 max-h-[280px] overflow-y-auto">
        {cached.map((item) => (
          <div
            key={item.id}
            className="group flex items-center gap-3 rounded-lg border border-slate-800 bg-slate-950/40 p-3 transition hover:border-slate-700"
          >
            {/* Icon */}
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-slate-800">
              <FileCode className="h-4 w-4 text-cyan-400" />
            </div>

            {/* Content */}
            <button
              type="button"
              onClick={() => onSelectResult(item.result)}
              className="flex-1 text-left"
            >
              <div className="text-sm font-medium text-slate-200 truncate">
                {item.functionName}
              </div>
              <div className="flex items-center gap-1 text-[10px] text-slate-500 mt-0.5">
                <Clock className="h-2.5 w-2.5" />
                {new Date(item.timestamp).toLocaleString('en-US', {
                  month: 'short',
                  day: 'numeric',
                  hour: '2-digit',
                  minute: '2-digit',
                })}
                {item.wasmFileName && (
                  <>
                    <span className="mx-1 text-slate-700">•</span>
                    <span className="truncate">{item.wasmFileName}</span>
                  </>
                )}
              </div>
            </button>

            {/* Delete button */}
            <button
              type="button"
              onClick={() => handleDelete(item.id)}
              aria-label="Delete cached analysis"
              className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-slate-500 opacity-0 transition hover:bg-rose-500/10 hover:text-rose-400 group-hover:opacity-100"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
