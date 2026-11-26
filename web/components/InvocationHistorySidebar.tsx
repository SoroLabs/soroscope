'use client';

import { useEffect, useMemo, useState } from 'react';
import { ChevronLeft, ChevronRight, Clock, CheckCircle, XCircle, Trash2 } from 'lucide-react';
import { useInvocationHistory } from './InnovocationHistory';
import { paginate } from '../lib/paginationUtils';
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
  const [page, setPage] = useState(1);
  const PER_PAGE = 20;
  const pagination = useMemo(() => paginate(history, page, PER_PAGE), [history, page]);
  const currentPage = pagination.page;
  const totalPages = pagination.totalPages;
  const pageItems = pagination.items;
  useEffect(() => {
    if (page > totalPages) setPage(totalPages);
  }, [page, totalPages]);

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
              pageItems.map((item, index) => {
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
                      borderBottom: index < pageItems.length - 1 ? '1px solid #21262d' : 'none',
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

          {/* Pagination */}
          {history.length > 20 && (
            <div
              style={{
                padding: '8px 14px',
                borderTop: '1px solid #30363d',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                flexShrink: 0,
              }}
            >
              <button
                onClick={() => setPage((p) => Math.max(1, p - 1))}
                disabled={currentPage <= 1}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: currentPage <= 1 ? 'default' : 'pointer',
                  color: currentPage <= 1 ? '#484f58' : 'var(--text-secondary)',
                  fontSize: '12px',
                }}
              >
                Previous
              </button>
              <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                Page {currentPage} of {totalPages}
              </span>
              <button
                onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                disabled={currentPage >= totalPages}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: currentPage >= totalPages ? 'default' : 'pointer',
                  color: currentPage >= totalPages ? '#484f58' : 'var(--text-secondary)',
                  fontSize: '12px',
                }}
              >
                Next
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
