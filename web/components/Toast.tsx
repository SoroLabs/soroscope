'use client';

import React, { useEffect } from 'react';

export interface ToastProps {
  message: string;
  type?: 'error' | 'success' | 'info';
  onClose: () => void;
  duration?: number;
}

export function Toast({ message, type = 'error', onClose, duration = 6000 }: ToastProps) {
  useEffect(() => {
    if (duration > 0) {
      const timer = setTimeout(() => {
        onClose();
      }, duration);
      return () => clearTimeout(timer);
    }
  }, [onClose, duration]);

  const colors = {
    error: {
      bg: '#161b22',
      border: '#fb8500',
      text: '#f0883e',
      badgeBg: '#2d1810',
    },
    success: {
      bg: '#161b22',
      border: '#00d9ff',
      text: '#00d9ff',
      badgeBg: '#0d2538',
    },
    info: {
      bg: '#161b22',
      border: '#58a6ff',
      text: '#58a6ff',
      badgeBg: '#10243e',
    },
  };

  const theme = colors[type];

  return (
    <div
      role="alert"
      style={{
        position: 'fixed',
        bottom: '24px',
        right: '24px',
        zIndex: 1000,
        minWidth: '320px',
        maxWidth: '480px',
        backgroundColor: theme.bg,
        border: `1px solid ${theme.border}`,
        borderRadius: '8px',
        padding: '16px',
        boxShadow: '0 8px 24px rgba(0, 0, 0, 0.5)',
        display: 'flex',
        alignItems: 'flex-start',
        gap: '12px',
        animation: 'slideIn 0.3s ease-out',
      }}
    >
      <div style={{ flex: 1 }}>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            marginBottom: '6px',
          }}
        >
          <span
            style={{
              fontSize: '11px',
              fontWeight: '700',
              textTransform: 'uppercase',
              letterSpacing: '0.5px',
              color: theme.text,
              backgroundColor: theme.badgeBg,
              padding: '2px 8px',
              borderRadius: '4px',
              border: `1px solid ${theme.border}`,
              fontFamily: 'monospace',
            }}
          >
            {type === 'error' ? 'Invocation Error' : type === 'success' ? 'Invocation Success' : 'Notice'}
          </span>
        </div>
        <p
          style={{
            margin: '0',
            fontSize: '13px',
            color: '#c9d1d9',
            lineHeight: '1.4',
            fontFamily: 'monospace, sans-serif',
            wordBreak: 'break-word',
          }}
        >
          {message}
        </p>
      </div>
      <button
        onClick={onClose}
        style={{
          background: 'none',
          border: 'none',
          color: '#8b949e',
          fontSize: '18px',
          cursor: 'pointer',
          padding: '0 4px',
          lineHeight: '1',
        }}
        aria-label="Close notification"
      >
        ×
      </button>
    </div>
  );
}
