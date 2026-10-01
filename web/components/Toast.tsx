'use client';

import React, { useEffect, useRef, useState } from 'react';
import {
  DEFAULT_TOAST_DURATION_MS,
  SWIPE_DISMISS_THRESHOLD_PX,
  getVisibleToasts,
} from '../lib/toastQueue.cjs';

export type ToastType = 'error' | 'success' | 'warning' | 'info';

export interface ToastItem {
  id: string;
  message: string;
  type: ToastType;
}

export interface ToastProps {
  message: string;
  type?: ToastType;
  onClose: () => void;
  duration?: number;
}

const EXIT_DURATION_MS = 180;

const COLORS: Record<ToastType, { border: string; text: string; badgeBg: string; title: string }> = {
  error: { border: '#fb8500', text: '#f0883e', badgeBg: '#2d1810', title: 'Error' },
  success: { border: '#00d9ff', text: '#00d9ff', badgeBg: '#0d2538', title: 'Success' },
  warning: { border: '#eab308', text: '#eab308', badgeBg: '#30270c', title: 'Warning' },
  info: { border: '#58a6ff', text: '#58a6ff', badgeBg: '#10243e', title: 'Info' },
};

export function Toast({ message, type = 'error', onClose, duration = DEFAULT_TOAST_DURATION_MS }: ToastProps) {
  const [visible, setVisible] = useState(false);
  const onCloseRef = useRef(onClose);
  const closeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const touchStartXRef = useRef<number | null>(null);
  const theme = COLORS[type];

  onCloseRef.current = onClose;

  useEffect(() => {
    const frame = requestAnimationFrame(() => setVisible(true));
    let exitTimer: ReturnType<typeof setTimeout> | undefined;
    let dismissTimer: ReturnType<typeof setTimeout> | undefined;

    if (duration > 0) {
      exitTimer = setTimeout(() => {
        setVisible(false);
        dismissTimer = setTimeout(() => onCloseRef.current(), EXIT_DURATION_MS);
      }, Math.max(0, duration - EXIT_DURATION_MS));
    }

    return () => {
      cancelAnimationFrame(frame);
      if (exitTimer) clearTimeout(exitTimer);
      if (dismissTimer) clearTimeout(dismissTimer);
      if (closeTimerRef.current) clearTimeout(closeTimerRef.current);
    };
  }, [duration]);

  const dismiss = () => {
    if (closeTimerRef.current) return;
    setVisible(false);
    closeTimerRef.current = setTimeout(() => onCloseRef.current(), EXIT_DURATION_MS);
  };

  return (
    <div
      role="alert"
      onTouchStart={(event) => {
        touchStartXRef.current = event.touches[0]?.clientX ?? null;
      }}
      onTouchEnd={(event) => {
        const startX = touchStartXRef.current;
        const endX = event.changedTouches[0]?.clientX;
        touchStartXRef.current = null;
        if (startX !== null && endX !== undefined && Math.abs(endX - startX) >= SWIPE_DISMISS_THRESHOLD_PX) dismiss();
      }}
      style={{
        minWidth: 'min(320px, calc(100vw - 32px))',
        maxWidth: '480px',
        backgroundColor: '#161b22',
        border: `1px solid ${theme.border}`,
        borderRadius: '8px',
        padding: '16px',
        boxShadow: '0 8px 24px rgba(0, 0, 0, 0.5)',
        display: 'flex',
        alignItems: 'flex-start',
        gap: '12px',
        opacity: visible ? 1 : 0,
        transform: visible ? 'translateX(0)' : 'translateX(20px)',
        transition: `opacity ${EXIT_DURATION_MS}ms ease, transform ${EXIT_DURATION_MS}ms ease`,
        pointerEvents: 'auto',
        touchAction: 'pan-y',
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
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
            {theme.title}
          </span>
        </div>
        <p
          style={{
            margin: 0,
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
        type="button"
        onClick={dismiss}
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

export function ToastStack({
  toasts,
  onClose,
  duration = DEFAULT_TOAST_DURATION_MS,
}: {
  toasts: ToastItem[];
  onClose: (id: string) => void;
  duration?: number;
}) {
  const visibleToasts = getVisibleToasts(toasts);

  if (visibleToasts.length === 0) return null;

  return (
    <div
      aria-label="Notifications"
      style={{
        position: 'fixed',
        bottom: '24px',
        right: '24px',
        zIndex: 60,
        display: 'flex',
        flexDirection: 'column-reverse',
        alignItems: 'flex-end',
        gap: '12px',
        pointerEvents: 'none',
      }}
    >
      {visibleToasts.map((toast) => (
        <Toast
          key={toast.id}
          message={toast.message}
          type={toast.type}
          duration={duration}
          onClose={() => onClose(toast.id)}
        />
      ))}
    </div>
  );
}
