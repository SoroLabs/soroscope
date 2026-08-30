'use client';

import { useState } from 'react';
import { ChevronLeft, ChevronRight, Clock, CheckCircle, XCircle, Trash2 } from 'lucide-react';
import { useInvocationHistory } from './InnovocationHistory';
import type { InvocationResult } from '../lib/sorobantypes';

interface InvocationHistorySidebarProps {
  onSelectResult: (result: InvocationResult) => void;
}

function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

function formatDate(timestamp: number): string {
  const date = new Date(timestamp);
  const today = new Date();
  if (date.toDateString() === today.toDateString()) return 'Today';
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === yesterday.toDateString()) return 'Yesterday';
  return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

export function InvocationHistorySidebar({ onSelectResult }: InvocationHistorySidebarProps) {
  const [isOpen, setIsOpen] = useState(true);
  const { history, clearHistory } = useInvocationHistory();

  return (
    <div style={{ position: 'relative', display: 'flex', alignItems: 'flex-start' }}>
      {/* Toggle button */}
      <button
        onClick={() => setIsOpen((prev) => !prev)}
        title={isOpen ? 'Hide history' : 'Show history'}
        style={{
          position: 'absolute',
          top: '16px',
          left: '-14px',
          zIndex: 10,
          width: '28px',
          height: '28px',
          borderRadius: '50%',
          backgroundColor: 'var(--bg-card)',
          border: '1px solid #30363d',
          cursor: 'pointer',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          color: 'var(--text-secondary)',
          flexShrink: 0,
        }}
      >
        {isOpen ? <ChevronRight size={14} /> : <ChevronLeft size={14} />}
      </button>

      {isOpen && (
        <div
          style={{
            width: '248px',
            backgroundColor: 'var(--bg-card)',
            border: '1px solid #30363d',
            borderRadius: '8px',
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden',
            maxHeight: '680px',
          }}
        >
          {/* Header */}
          <div
            style={{
              padding: '12px 14px',
              borderBottom: '1px solid #30363d',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              flexShrink: 0,
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '7px' }}>
              <Clock size={13} color="#8b949e" />
              <span style={{ fontSize: '13px', fontWeight: '600', color: 'var(--text-primary)' }}>
                History
              </span>
              {history.length > 0 && (
                <span
                  style={{
                    fontSize: '11px',
                    backgroundColor: '#21262d',
                    color: 'var(--text-secondary)',
                    borderRadius: '10px',
                    padding: '1px 6px',
                    lineHeight: '16px',
                  }}
                >
                  {history.length}
                </span>
              )}
            </div>

            {history.length > 0 && (
              <button
                onClick={clearHistory}
                title="Clear all history"
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  color: 'var(--text-secondary)',
                  padding: '2px',
                  display: 'flex',
                  alignItems: 'center',
                  borderRadius: '4px',
                }}
                onMouseEnter={(e) => { e.currentTarget.style.color = '#f85149'; }}
                onMouseLeave={(e) => { e.currentTarget.style.color = '#8b949e'; }}
              >
                <Trash2 size={12} />
              </button>
            )}
          </div>

          {/* List */}
          <div style={{ overflowY: 'auto', flex: 1 }}>
            {history.length === 0 ? (
              <div
                style={{
                  padding: '40px 16px',
                  textAlign: 'center',
                  color: 'var(--text-secondary)',
                  fontSize: '12px',
                }}
              >
                <Clock size={22} color="#30363d" style={{ margin: '0 auto 10px', display: 'block' }} />
                No analyses yet
              </div>
            ) : (
              history.map((item, index) => {
                const report = item.analysisReport ?? (item as any).resourceCost;

                return (
                  <button
                    key={item.id}
                    onClick={() => onSelectResult(item)}
                    style={{
                      width: '100%',
                      padding: '10px 14px',
                      backgroundColor: 'transparent',
                      border: 'none',
                      borderBottom: index < history.length - 1 ? '1px solid #21262d' : 'none',
                      textAlign: 'left',
                      cursor: 'pointer',
                      display: 'block',
                      transition: 'background-color 0.12s',
                    }}
                    onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = '#1c2128'; }}
                    onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
                  >
                    <div style={{ display: 'flex', gap: '8px', alignItems: 'flex-start' }}>
                      <div style={{ paddingTop: '1px', flexShrink: 0 }}>
                        {item.success ? (
                          <CheckCircle size={13} color="#3fb950" />
                        ) : (
                          <XCircle size={13} color="#f85149" />
                        )}
                      </div>

                      <div style={{ flex: 1, minWidth: 0 }}>
                        <p
                          style={{
                            margin: '0 0 2px 0',
                            fontSize: '12px',
                            fontWeight: '500',
                            color: 'var(--text-primary)',
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                            whiteSpace: 'nowrap',
                          }}
                        >
                          {item.functionName}
                        </p>

                        <p style={{ margin: '0', fontSize: '11px', color: 'var(--text-secondary)' }}>
                          {formatDate(item.timestamp)} · {formatTime(item.timestamp)}
                        </p>

                        {report && item.success && (
                          <p
                            style={{
                              margin: '3px 0 0 0',
                              fontSize: '11px',
                              color: '#00d9ff',
                              fontFamily: 'monospace',
                            }}
                          >
                            {(report.cpu_instructions / 1_000_000).toFixed(1)}M CPU
                            {' · '}
                            {(report.ram_bytes / 1024).toFixed(0)} KB
                          </p>
                        )}

                        {item.error && (
                          <p
                            style={{
                              margin: '3px 0 0 0',
                              fontSize: '11px',
                              color: '#f85149',
                              overflow: 'hidden',
                              textOverflow: 'ellipsis',
                              whiteSpace: 'nowrap',
                            }}
                          >
                            {item.error}
                          </p>
                        )}
                      </div>
                    </div>
                  </button>
                );
              })
            )}
          </div>
        </div>
      )}
    </div>
  );
}
