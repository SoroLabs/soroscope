'use client';

import { useEffect, useState } from 'react';
import type { InvocationResult } from '../lib/sorobantypes';

interface InvocationHistoryProps {
  onSelectResult: (result: InvocationResult) => void;
}

const HISTORY_KEY = 'soroban-invocation-history';
const MAX_HISTORY = 10;

export function useInvocationHistory() {
  const [history, setHistory] = useState<InvocationResult[]>([]);
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
    const saved = localStorage.getItem(HISTORY_KEY);
    if (saved) {
      try {
        setHistory(JSON.parse(saved));
      } catch {
        setHistory([]);
      }
    }
  }, []);

  const addToHistory = (result: InvocationResult) => {
    setHistory((prev) => {
      const updated = [result, ...prev].slice(0, MAX_HISTORY);
      if (mounted) {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(updated));
      }
      return updated;
    });
  };

  const clearHistory = () => {
    setHistory([]);
    if (mounted) {
      localStorage.removeItem(HISTORY_KEY);
    }
  };

  return { history, addToHistory, clearHistory, mounted };
}

export function InvocationHistory({ onSelectResult }: InvocationHistoryProps) {
  const { history, clearHistory } = useInvocationHistory();

  if (history.length === 0) {
    return (
      <div
        style={{
          padding: '24px',
          backgroundColor: '#0d1117',
          borderRadius: '8px',
          textAlign: 'center',
          color: '#8b949e',
          border: '1px solid #30363d',
        }}
      >
        <p>No invocation history yet.</p>
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
        <h3 style={{ margin: '0', fontSize: '16px', fontWeight: '600', color: '#c9d1d9' }}>
          Recent Invocations
        </h3>
        <button
          onClick={clearHistory}
          style={{
            padding: '6px 12px',
            backgroundColor: '#161b22',
            border: '1px solid #30363d',
            borderRadius: '6px',
            fontSize: '12px',
            cursor: 'pointer',
            color: '#8b949e',
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
              backgroundColor: '#0d1117',
              border: '1px solid #30363d',
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
                <p style={{ margin: '0 0 4px 0', fontSize: '14px', fontWeight: '500', color: '#c9d1d9' }}>
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
                <p style={{ margin: '0', fontSize: '12px', color: '#8b949e' }}>
                  {new Date(item.timestamp).toLocaleTimeString()}
                </p>
              </div>
              <div style={{ textAlign: 'right', fontSize: '12px', color: '#8b949e' }}>
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
    </div>
  );
}
