'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { InvocationResult } from '../lib/sorobantypes';
import { getEncryptedLocalStorage } from '../lib/encryptedStorage';

interface InvocationHistoryProps {
  onSelectResult: (result: InvocationResult) => void;
}

const HISTORY_KEY = 'soroban-invocation-history';
const MAX_HISTORY = 10;

const LOG_PAGE_SIZE = 50;
const LOG_ROW_HEIGHT = 64;
const LOG_VIEWPORT_HEIGHT = 320;
const LOG_OVERSCAN = 4;
const LOG_TOTAL_ENTRIES = 1000;

type LogLevel = 'Info' | 'Warn' | 'Error';

interface TelemetryLogEntry {
  id: string;
  level: LogLevel;
  message: string;
  timestamp: number;
}

const LOG_LEVELS: LogLevel[] = ['Info', 'Warn', 'Error'];

const LOG_LEVEL_COLORS: Record<LogLevel, string> = {
  Info: '#00d9ff',
  Warn: '#fb8500',
  Error: '#ff5c5c',
};

function createMockTelemetryLogs(count: number): TelemetryLogEntry[] {
  const base = Date.now();
  const entries: TelemetryLogEntry[] = [];
  for (let i = 0; i < count; i += 1) {
    const level = LOG_LEVELS[i % LOG_LEVELS.length];
    entries.push({
      id: `log-${i}`,
      level,
      message: `[${level}] telemetry event #${i} processed`,
      timestamp: base - i * 1000,
    });
  }
  return entries;
}

export function useInvocationHistory() {
  const [history, setHistory] = useState<InvocationResult[]>([]);
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
    let active = true;

    async function restoreHistory() {
      try {
        const saved = await getEncryptedLocalStorage()?.getItem(HISTORY_KEY);
        if (active && saved) {
          setHistory(JSON.parse(saved));
        }
      } catch {
        if (active) {
          setHistory([]);
        }
      }
    }

    void restoreHistory();
    return () => {
      active = false;
    };
  }, []);

  const addToHistory = (result: InvocationResult) => {
    setHistory((prev) => {
      const updated = [result, ...prev].slice(0, MAX_HISTORY);
      if (mounted) {
        void getEncryptedLocalStorage()
          ?.setItem(HISTORY_KEY, JSON.stringify(updated))
          .catch((error: unknown) => {
            console.warn('Failed to save invocation history:', error);
          });
      }
      return updated;
    });
  };

  const clearHistory = () => {
    setHistory([]);
    if (mounted) {
      getEncryptedLocalStorage()?.removeItem(HISTORY_KEY);
    }
  };

  return { history, addToHistory, clearHistory, mounted };
}

function TelemetryLogViewer() {
  const allLogs = useMemo(() => createMockTelemetryLogs(LOG_TOTAL_ENTRIES), []);
  const [levelFilter, setLevelFilter] = useState<LogLevel | 'All'>('All');
  const [visibleCount, setVisibleCount] = useState(LOG_PAGE_SIZE);
  const [scrollTop, setScrollTop] = useState(0);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  const filteredLogs = useMemo(
    () => (levelFilter === 'All' ? allLogs : allLogs.filter((log) => log.level === levelFilter)),
    [allLogs, levelFilter],
  );

  useEffect(() => {
    setVisibleCount(LOG_PAGE_SIZE);
    setScrollTop(0);
  }, [levelFilter]);

  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel) {
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setVisibleCount((prev) => Math.min(prev + LOG_PAGE_SIZE, filteredLogs.length));
        }
      },
      { rootMargin: '120px' },
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [filteredLogs.length]);

  const loadedLogs = filteredLogs.slice(0, visibleCount);

  const startIndex = Math.max(0, Math.floor(scrollTop / LOG_ROW_HEIGHT) - LOG_OVERSCAN);
  const endIndex = Math.min(
    loadedLogs.length,
    Math.ceil((scrollTop + LOG_VIEWPORT_HEIGHT) / LOG_ROW_HEIGHT) + LOG_OVERSCAN,
  );
  const virtualLogs = loadedLogs.slice(startIndex, endIndex);

  const handleScroll = useCallback((event: React.UIEvent<HTMLDivElement>) => {
    setScrollTop(event.currentTarget.scrollTop);
  }, []);

  return (
    <div style={{ marginTop: '24px' }}>
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '12px',
          gap: '12px',
          flexWrap: 'wrap',
        }}
      >
        <h3 style={{ margin: '0', fontSize: '16px', fontWeight: '600', color: 'var(--text-primary)' }}>
          Telemetry Logs
        </h3>
        <div style={{ display: 'flex', gap: '6px' }}>
          {(['All', ...LOG_LEVELS] as const).map((level) => (
            <button
              key={level}
              onClick={() => setLevelFilter(level)}
              style={{
                padding: '4px 10px',
                backgroundColor: levelFilter === level ? 'var(--bg-card)' : 'transparent',
                border: '1px solid var(--border-default)',
                borderRadius: '6px',
                fontSize: '12px',
                cursor: 'pointer',
                color: levelFilter === level ? 'var(--text-primary)' : 'var(--text-secondary)',
              }}
            >
              {level}
            </button>
          ))}
        </div>
      </div>

      <div
        onScroll={handleScroll}
        style={{
          height: `${LOG_VIEWPORT_HEIGHT}px`,
          overflowY: 'auto',
          border: '1px solid var(--border-default)',
          borderRadius: '8px',
          backgroundColor: 'var(--bg-elevated)',
        }}
      >
        <div style={{ height: `${loadedLogs.length * LOG_ROW_HEIGHT}px`, position: 'relative' }}>
          {virtualLogs.map((log, index) => (
            <div
              key={log.id}
              style={{
                position: 'absolute',
                top: `${(startIndex + index) * LOG_ROW_HEIGHT}px`,
                left: 0,
                right: 0,
                height: `${LOG_ROW_HEIGHT}px`,
                display: 'flex',
                alignItems: 'center',
                gap: '12px',
                padding: '0 12px',
                borderBottom: '1px solid var(--border-default)',
                fontSize: '13px',
              }}
            >
              <span style={{ color: LOG_LEVEL_COLORS[log.level], fontWeight: '600', width: '48px' }}>
                {log.level}
              </span>
              <span style={{ flex: 1, color: 'var(--text-primary)' }}>{log.message}</span>
              <span style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                {new Date(log.timestamp).toLocaleTimeString()}
              </span>
            </div>
          ))}
        </div>
        {visibleCount < filteredLogs.length && (
          <div ref={sentinelRef} style={{ height: '1px' }} />
        )}
      </div>
    </div>
  );
}

export function InvocationHistory({ onSelectResult }: InvocationHistoryProps) {
  const { history, clearHistory } = useInvocationHistory();

  if (history.length === 0) {
    return (
      <div>
        <div
          style={{
            padding: '24px',
            backgroundColor: 'var(--bg-elevated)',
            borderRadius: '8px',
            textAlign: 'center',
            color: 'var(--text-secondary)',
            border: '1px solid var(--border-default)',
          }}
        >
          <p>No invocation history yet.</p>
        </div>
        <TelemetryLogViewer />
      </div>
    );
  }

  return (
    <div>
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '12px',
        }}
      >
        <h3 style={{ margin: '0', fontSize: '16px', fontWeight: '600', color: 'var(--text-primary)' }}>
          Recent Invocations
        </h3>
        <button
          onClick={clearHistory}
          style={{
            padding: '6px 12px',
            backgroundColor: 'var(--bg-card)',
            border: '1px solid var(--border-default)',
            borderRadius: '6px',
            fontSize: '12px',
            cursor: 'pointer',
            color: 'var(--text-secondary)',
          }}
        >
          Clear History
        </button>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
        {history.map((item) => (
          <button
            key={item.id}
            onClick={() => onSelectResult(item)}
            style={{
              padding: '12px',
              backgroundColor: 'var(--bg-elevated)',
              border: '1px solid var(--border-default)',
              borderRadius: '6px',
              textAlign: 'left',
              cursor: 'pointer',
              transition: 'all 0.2s',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = '#161b22';
              e.currentTarget.style.borderColor = '#00d9ff';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = '#0d1117';
              e.currentTarget.style.borderColor = '#30363d';
            }}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
              }}
            >
              <div>
                <p style={{ margin: '0 0 4px 0', fontSize: '14px', fontWeight: '500', color: 'var(--text-primary)' }}>
                  <span
                    style={{
                      color: item.success ? '#00d9ff' : '#fb8500',
                      marginRight: '8px',
                    }}
                  >
                    {item.success ? '✓' : '✗'}
                  </span>
                  {item.functionName}
                </p>
                <p style={{ margin: '0', fontSize: '12px', color: 'var(--text-secondary)' }}>
                  {new Date(item.timestamp).toLocaleTimeString()}
                </p>
              </div>
              <div style={{ textAlign: 'right', fontSize: '12px', color: 'var(--text-secondary)' }}>
                {item.error ? (
                  <span style={{ color: '#fb8500' }}>Error</span>
                ) : (
                  <span style={{ color: '#00d9ff' }}>Success</span>
                )}
              </div>
            </div>
          </button>
        ))}
      </div>

      <TelemetryLogViewer />
    </div>
  );
}
