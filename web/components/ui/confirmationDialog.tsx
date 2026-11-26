'use client';

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { CheckCircle2, HelpCircle, X, XCircle } from 'lucide-react';

/**
 * ConfirmationDialog - A simpler confirmation dialog for quick yes/no decisions.
 * Lightweight alternative to AlertDialog for straightforward confirmations.
 */
export interface ConfirmationDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: 'default' | 'danger' | 'success';
  isLoading?: boolean;
}

const VARIANT_CONFIG = {
  default: {
    icon: HelpCircle,
    iconClass: 'text-cyan-400',
    confirmClass: 'bg-cyan-500 hover:bg-cyan-600 text-slate-950',
  },
  danger: {
    icon: XCircle,
    iconClass: 'text-red-400',
    confirmClass: 'bg-red-500 hover:bg-red-600 text-white',
  },
  success: {
    icon: CheckCircle2,
    iconClass: 'text-emerald-400',
    confirmClass: 'bg-emerald-500 hover:bg-emerald-600 text-white',
  },
};

export function ConfirmationDialog({
  isOpen,
  onClose,
  onConfirm,
  title,
  message,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  variant = 'default',
  isLoading = false,
}: ConfirmationDialogProps) {
  const [isLeaving, setIsLeaving] = useState(false);
  const confirmRef = useRef<HTMLButtonElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  const handleClose = useCallback(() => {
    setIsLeaving(true);
    setTimeout(() => {
      setIsLeaving(false);
      onClose();
    }, 150);
  }, [onClose]);

  const handleConfirm = useCallback(() => {
    onConfirm();
  }, [onConfirm]);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === 'Escape') {
        handleClose();
        return;
      }

      if (event.key === 'Tab') {
        const focusableElements = dialogRef.current?.querySelectorAll<HTMLElement>(
          'button:enabled, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
        );
        if (!focusableElements || focusableElements.length === 0) return;

        const firstElement = focusableElements[0];
        const lastElement = focusableElements[focusableElements.length - 1];

        if (event.shiftKey && document.activeElement === firstElement) {
          event.preventDefault();
          lastElement.focus();
        } else if (!event.shiftKey && document.activeElement === lastElement) {
          event.preventDefault();
          firstElement.focus();
        }
      }
    },
    [handleClose],
  );

  useEffect(() => {
    if (isOpen) {
      previousFocusRef.current = document.activeElement as HTMLElement;
      document.body.style.overflow = 'hidden';

      const timer = setTimeout(() => {
        confirmRef.current?.focus();
      }, 50);

      return () => {
        clearTimeout(timer);
        document.body.style.overflow = '';
      };
    }
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen && previousFocusRef.current) {
      previousFocusRef.current.focus();
    }
  }, [isOpen]);

  const config = VARIANT_CONFIG[variant];
  const Icon = config.icon;

  if (!isOpen && !isLeaving) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center px-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirmation-dialog-title"
      aria-describedby="confirmation-dialog-message"
    >
      {/* Backdrop */}
      <div
        className={`absolute inset-0 bg-slate-950/80 backdrop-blur-sm transition-opacity duration-150 ${
          isLeaving ? 'opacity-0' : 'opacity-100'
        }`}
        onClick={handleClose}
        aria-hidden="true"
      />

      {/* Dialog panel */}
      <div
        ref={dialogRef}
        onKeyDown={handleKeyDown}
        className={`relative w-full max-w-sm rounded-2xl border border-slate-700 bg-slate-900 shadow-2xl transition-all duration-150 ${
          isLeaving ? 'opacity-0 scale-95' : 'opacity-100 scale-100'
        }`}
      >
        {/* Content */}
        <div className="p-6">
          {/* Icon */}
          <div className={`mb-4 flex justify-center ${config.iconClass}`}>
            <Icon className="h-10 w-10" />
          </div>

          {/* Title */}
          <h2
            id="confirmation-dialog-title"
            className="text-center text-lg font-semibold text-white"
          >
            {title}
          </h2>

          {/* Message */}
          <p
            id="confirmation-dialog-message"
            className="mt-2 text-center text-sm text-slate-400"
          >
            {message}
          </p>

          {/* Actions */}
          <div className="mt-6 flex gap-3">
            <button
              type="button"
              onClick={handleClose}
              disabled={isLoading}
              className="flex-1 min-h-[40px] rounded-lg border border-slate-700 bg-slate-800 px-4 text-sm font-medium text-slate-300 transition-colors hover:bg-slate-700 hover:text-white focus:outline-none focus:ring-2 focus:ring-cyan-500/50 disabled:opacity-50"
            >
              {cancelLabel}
            </button>
            <button
              ref={confirmRef}
              type="button"
              onClick={handleConfirm}
              disabled={isLoading}
              className={`flex-1 min-h-[40px] rounded-lg px-4 text-sm font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-offset-slate-900 disabled:opacity-50 ${config.confirmClass}`}
            >
              {isLoading ? (
                <span className="flex items-center justify-center gap-2">
                  <svg className="h-4 w-4 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                    <circle cx="12" cy="12" r="10" strokeOpacity={0.25} />
                    <path d="M12 2a10 10 0 0 1 10 10" strokeLinecap="round" />
                  </svg>
                  Processing...
                </span>
              ) : (
                confirmLabel
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default ConfirmationDialog;