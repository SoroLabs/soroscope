"use client";

import React, { createContext, useContext, useState, useCallback } from "react";
import {
  ConfirmationModal,
  ConfirmationVariant,
  type ConfirmationModalProps,
} from "../components/ConfirmationModal";

interface ConfirmationOptions
  extends Omit<
    ConfirmationModalProps,
    "isOpen" | "onClose" | "onConfirm" | "isLoading"
  > {
  onConfirm?: () => void | Promise<void>;
}

interface ConfirmationDialogContextValue {
  confirm: (options: ConfirmationOptions) => Promise<boolean>;
}

const ConfirmationDialogContext = createContext<ConfirmationDialogContextValue | null>(
  null
);

export function ConfirmationDialogProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [dialogState, setDialogState] = useState<{
    isOpen: boolean;
    options: ConfirmationOptions | null;
    resolve: ((value: boolean) => void) | null;
  }>({
    isOpen: false,
    options: null,
    resolve: null,
  });

  const [isLoading, setIsLoading] = useState(false);

  const confirm = useCallback((options: ConfirmationOptions): Promise<boolean> => {
    return new Promise((resolve) => {
      setDialogState({
        isOpen: true,
        options,
        resolve,
      });
    });
  }, []);

  const handleClose = useCallback(() => {
    setDialogState((prev) => {
      if (prev.resolve) {
        prev.resolve(false);
      }
      return {
        ...prev,
        isOpen: false,
        options: null,
        resolve: null,
      };
    });
    setIsLoading(false);
  }, []);

  const handleConfirm = useCallback(async () => {
    if (!dialogState.options) return;

    setIsLoading(true);

    try {
      if (dialogState.options.onConfirm) {
        await dialogState.options.onConfirm();
      }
      setDialogState((prev) => {
        if (prev.resolve) {
          prev.resolve(true);
        }
        return {
          ...prev,
          isOpen: false,
          options: null,
          resolve: null,
        };
      });
    } catch (error) {
      console.error("Confirmation action failed:", error);
      setDialogState((prev) => {
        if (prev.resolve) {
          prev.resolve(false);
        }
        return {
          ...prev,
          isOpen: false,
          options: null,
          resolve: null,
        };
      });
      setIsLoading(false);
    }
  }, [dialogState.options]);

  return (
    <ConfirmationDialogContext.Provider value={{ confirm }}>
      {children}
      {dialogState.isOpen && dialogState.options && (
        <ConfirmationModal
          isOpen={dialogState.isOpen}
          onClose={handleClose}
          onConfirm={handleConfirm}
          title={dialogState.options.title}
          message={dialogState.options.message}
          confirmText={dialogState.options.confirmText}
          cancelText={dialogState.options.cancelText}
          variant={dialogState.options.variant}
          isDestructive={dialogState.options.isDestructive}
          metrics={dialogState.options.metrics}
          isLoading={isLoading}
        />
      )}
    </ConfirmationDialogContext.Provider>
  );
}

export function useConfirmationDialog() {
  const context = useContext(ConfirmationDialogContext);

  if (!context) {
    throw new Error(
      "useConfirmationDialog must be used within a ConfirmationDialogProvider"
    );
  }

  return context;
}

// Convenience hook for common confirmation scenarios
export function useConfirm() {
  const { confirm } = useConfirmationDialog();

  return {
    confirm,
    // Pre-configured confirmations for common use cases
    confirmDanger: (options: Omit<ConfirmationOptions, "variant" | "isDestructive">) =>
      confirm({ ...options, variant: "danger", isDestructive: true }),
    confirmWarning: (options: Omit<ConfirmationOptions, "variant">) =>
      confirm({ ...options, variant: "warning" }),
    confirmInfo: (options: Omit<ConfirmationOptions, "variant">) =>
      confirm({ ...options, variant: "info" }),
  };
}
