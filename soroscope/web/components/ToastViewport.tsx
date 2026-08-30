'use client';

import React, { createContext, useCallback, useContext, useMemo, useState } from 'react';

const { buildTransactionToast } = require('../lib/transactionToasts');

export type TransactionToastPhase = 'signing' | 'submitting' | 'success' | 'failed';

export interface TransactionToastPayload {
  phase: TransactionToastPhase;
  message?: string;
  txHash?: string;
}

export interface TransactionToast extends TransactionToastPayload {
  id: string;
  title: string;
  message: string;
}

interface ToastContextValue {
  toasts: TransactionToast[];
  showToast: (payload: TransactionToastPayload) => string;
  dismissToast: (id: string) => void;
}

const ToastContext = createContext<ToastContextValue | undefined>(undefined);

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<TransactionToast[]>([]);

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((toast) => toast.id !== id));
  }, []);

  const showToast = useCallback((payload: TransactionToastPayload) => {
    const toast = buildTransactionToast(payload.phase, payload);
    setToasts((prev) => {
      const filtered = prev.filter((item) => item.phase !== payload.phase);
      return [...filtered, toast].slice(-4);
    });

    if (payload.phase === 'success' || payload.phase === 'failed') {
      window.setTimeout(() => dismissToast(toast.id), 4000);
    }

    return toast.id;
  }, [dismissToast]);

  const value = useMemo(() => ({ toasts, showToast, dismissToast }), [toasts, showToast, dismissToast]);

  return (
    <ToastContext.Provider value={value}>
      {children}
      <ToastViewport />
    </ToastContext.Provider>
  );
}

export function useTransactionToasts() {
  const context = useContext(ToastContext);
  if (!context) {
    throw new Error('useTransactionToasts must be used within a ToastProvider');
  }
  return context;
}

function ToastViewport() {
  const { toasts, dismissToast } = useTransactionToasts();

  if (toasts.length === 0) {
    return null;
  }

  return (
    <div
      aria-live="polite"
      style={{
        position: 'fixed',
        right: '16px',
        bottom: '16px',
        zIndex: 200,
        display: 'flex',
        flexDirection: 'column',
        gap: '8px',
      }}
    >
      {toasts.map((toast) => (
        <div
          key={toast.id}
          style={{
            minWidth: '280px',
            maxWidth: '360px',
            borderRadius: '10px',
            padding: '12px 14px',
            backgroundColor: toast.phase === 'failed' ? '#3f1d1d' : toast.phase === 'success' ? '#103421' : '#161b22',
            border: `1px solid ${toast.phase === 'failed' ? '#fb8500' : toast.phase === 'success' ? '#34d399' : '#30363d'}`,
            color: '#f8fafc',
            boxShadow: '0 10px 30px rgba(0, 0, 0, 0.3)',
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', gap: '8px', alignItems: 'center' }}>
            <strong style={{ fontSize: '14px' }}>{toast.title}</strong>
            <button
              type="button"
              onClick={() => dismissToast(toast.id)}
              style={{ background: 'transparent', border: 'none', color: '#cbd5e1', cursor: 'pointer' }}
              aria-label="Dismiss notification"
            >
              ×
            </button>
          </div>
          <div style={{ marginTop: '6px', fontSize: '13px', color: '#cbd5e1', lineHeight: 1.4 }}>{toast.message}</div>
        </div>
      ))}
    </div>
  );
}
