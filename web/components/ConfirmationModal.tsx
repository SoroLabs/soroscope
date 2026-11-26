"use client";

import React, { useEffect, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { AlertTriangle, Info, CheckCircle, X } from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type ConfirmationVariant = "warning" | "danger" | "info" | "success";

export interface ConfirmationModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  variant?: ConfirmationVariant;
  isDestructive?: boolean;
  metrics?: {
    label: string;
    value: string;
  }[];
  isLoading?: boolean;
}

const variantStyles = {
  warning: {
    icon: AlertTriangle,
    iconBg: "bg-amber-900/30",
    iconColor: "text-amber-400",
    border: "border-amber-700",
    confirmBg: "bg-amber-600",
    confirmHover: "hover:bg-amber-700",
  },
  danger: {
    icon: AlertTriangle,
    iconBg: "bg-red-900/30",
    iconColor: "text-red-400",
    border: "border-red-700",
    confirmBg: "bg-red-600",
    confirmHover: "hover:bg-red-700",
  },
  info: {
    icon: Info,
    iconBg: "bg-blue-900/30",
    iconColor: "text-blue-400",
    border: "border-blue-700",
    confirmBg: "bg-blue-600",
    confirmHover: "hover:bg-blue-700",
  },
  success: {
    icon: CheckCircle,
    iconBg: "bg-emerald-900/30",
    iconColor: "text-emerald-400",
    border: "border-emerald-700",
    confirmBg: "bg-emerald-600",
    confirmHover: "hover:bg-emerald-700",
  },
};

export function ConfirmationModal({
  isOpen,
  onClose,
  onConfirm,
  title,
  message,
  confirmText = "Confirm",
  cancelText = "Cancel",
  variant = "warning",
  isDestructive = false,
  metrics = [],
  isLoading = false,
}: ConfirmationModalProps) {
  const modalRef = useRef<HTMLDivElement>(null);
  const confirmButtonRef = useRef<HTMLButtonElement>(null);
  const cancelButtonRef = useRef<HTMLButtonElement>(null);
  const previousActiveElement = useRef<HTMLElement | null>(null);

  const styles = variantStyles[variant];
  const Icon = styles.icon;

  // Focus management
  useEffect(() => {
    if (isOpen) {
      previousActiveElement.current = document.activeElement as HTMLElement;
      confirmButtonRef.current?.focus();
    } else if (previousActiveElement.current) {
      previousActiveElement.current.focus();
    }
  }, [isOpen]);

  // Escape key handler
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape" && isOpen && !isLoading) {
        onClose();
      }
    };

    if (isOpen) {
      document.addEventListener("keydown", handleEscape);
      return () => document.removeEventListener("keydown", handleEscape);
    }
  }, [isOpen, onClose, isLoading]);

  // Focus trap
  useEffect(() => {
    if (!isOpen) return;

    const handleTab = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;

      const focusableElements = modalRef.current?.querySelectorAll(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );

      if (!focusableElements || focusableElements.length === 0) return;

      const firstElement = focusableElements[0] as HTMLElement;
      const lastElement = focusableElements[
        focusableElements.length - 1
      ] as HTMLElement;

      if (e.shiftKey) {
        if (document.activeElement === firstElement) {
          e.preventDefault();
          lastElement.focus();
        }
      } else {
        if (document.activeElement === lastElement) {
          e.preventDefault();
          firstElement.focus();
        }
      }
    };

    document.addEventListener("keydown", handleTab);
    return () => document.removeEventListener("keydown", handleTab);
  }, [isOpen]);

  const handleConfirm = () => {
    onConfirm();
  };

  const handleCancel = () => {
    if (!isLoading) {
      onClose();
    }
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <>
          {/* Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm"
            onClick={handleCancel}
            aria-hidden="true"
          />

          {/* Modal */}
          <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
            <motion.div
              ref={modalRef}
              role="dialog"
              aria-modal="true"
              aria-labelledby="confirmation-title"
              aria-describedby="confirmation-message"
              initial={{ opacity: 0, scale: 0.95, y: 10 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.95, y: 10 }}
              transition={{ duration: 0.2, ease: "easeOut" }}
              className={cn(
                "w-full max-w-md bg-slate-800 rounded-2xl shadow-2xl border",
                styles.border,
                "overflow-hidden"
              )}
              onClick={(e) => e.stopPropagation()}
            >
              {/* Header */}
              <div className="flex items-start gap-4 p-6 pb-4">
                <div
                  className={cn(
                    "flex-shrink-0 w-12 h-12 rounded-full flex items-center justify-center",
                    styles.iconBg
                  )}
                >
                  <Icon className={cn("w-6 h-6", styles.iconColor)} />
                </div>

                <div className="flex-1 min-w-0">
                  <h3
                    id="confirmation-title"
                    className="text-lg font-semibold text-slate-100 mb-2"
                  >
                    {title}
                  </h3>
                  <p
                    id="confirmation-message"
                    className="text-sm text-slate-400 leading-relaxed"
                  >
                    {message}
                  </p>
                </div>

                <button
                  onClick={handleCancel}
                  disabled={isLoading}
                    className={cn(
                      "flex-shrink-0 p-1 rounded-lg text-slate-500 hover:text-slate-300 hover:bg-slate-700 transition-colors",
                      isLoading && "cursor-not-allowed opacity-50"
                    )}
                  aria-label="Close dialog"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>

              {/* Metrics Section */}
              {metrics.length > 0 && (
                <div className="px-6 pb-4">
                  <div className="bg-slate-900/50 rounded-lg p-4 space-y-2">
                    {metrics.map((metric, index) => (
                      <div
                        key={index}
                        className="flex items-center justify-between text-sm"
                      >
                        <span className="text-slate-400">{metric.label}</span>
                        <span className="font-medium text-slate-100">
                          {metric.value}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Footer */}
              <div className="flex items-center justify-end gap-3 p-6 pt-4 bg-slate-900/30 border-t border-slate-700">
                <button
                  ref={cancelButtonRef}
                  onClick={handleCancel}
                  disabled={isLoading}
                  className={cn(
                    "px-4 py-2.5 rounded-xl font-medium text-sm transition-colors",
                    "text-slate-300 hover:text-white hover:bg-slate-700",
                    isLoading && "cursor-not-allowed opacity-50"
                  )}
                >
                  {cancelText}
                </button>
                <button
                  ref={confirmButtonRef}
                  onClick={handleConfirm}
                  disabled={isLoading}
                  className={cn(
                    "px-4 py-2.5 rounded-xl font-medium text-sm text-white transition-colors flex items-center gap-2",
                    styles.confirmBg,
                    styles.confirmHover,
                    isDestructive && "bg-red-600 hover:bg-red-700",
                    isLoading && "cursor-not-allowed opacity-70"
                  )}
                >
                  {isLoading && (
                    <svg
                      className="animate-spin h-4 w-4"
                      xmlns="http://www.w3.org/2000/svg"
                      fill="none"
                      viewBox="0 0 24 24"
                    >
                      <circle
                        className="opacity-25"
                        cx="12"
                        cy="12"
                        r="10"
                        stroke="currentColor"
                        strokeWidth="4"
                      />
                      <path
                        className="opacity-75"
                        fill="currentColor"
                        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                      />
                    </svg>
                  )}
                  {confirmText}
                </button>
              </div>
            </motion.div>
          </div>
        </>
      )}
    </AnimatePresence>
  );
}
