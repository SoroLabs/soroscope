"use client";

import React, { useState, useEffect, useCallback, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Wifi, WifiOff, X } from "lucide-react";

type OfflineState = "online" | "offline" | "reconnecting";

/**
 * OfflineBanner - A dismissible top-of-page banner that monitors network connectivity.
 *
 * - Shows a warning banner when the browser detects the device is offline.
 * - Shows a "Back online! Reconnecting..." success banner briefly when connectivity returns.
 * - Can be dismissed manually via the close button (persists for the current session until
 *   the offline state is re-triggered).
 */
export function OfflineBanner() {
  const [offlineState, setOfflineState] = useState<OfflineState>(
    typeof navigator !== "undefined" && !navigator.onLine ? "offline" : "online"
  );
  const [dismissed, setDismissed] = useState(false);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mountedRef = useRef(true);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      mountedRef.current = false;
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
      }
    };
  }, []);

  const handleOnline = useCallback(() => {
    if (!mountedRef.current) return;
    // If we were offline, show the "back online" banner briefly
    if (offlineState === "offline") {
      setOfflineState("reconnecting");
      setDismissed(false);

      // Auto-dismiss after 3 seconds
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
      }
      reconnectTimerRef.current = setTimeout(() => {
        if (mountedRef.current) {
          setOfflineState("online");
          setDismissed(false);
        }
      }, 3000);
    }
  }, [offlineState]);

  const handleOffline = useCallback(() => {
    if (!mountedRef.current) return;
    setOfflineState("offline");
    setDismissed(false);

    // Clear any pending reconnect timer
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
  }, []);

  useEffect(() => {
    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);

    return () => {
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("offline", handleOffline);
    };
  }, [handleOnline, handleOffline]);

  // If online and not reconnecting, or dismissed, don't show anything
  if ((offlineState === "online") || dismissed) {
    return null;
  }

  const isOffline = offlineState === "offline";
  const isReconnecting = offlineState === "reconnecting";

  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={offlineState}
        initial={{ height: 0, opacity: 0 }}
        animate={{ height: "auto", opacity: 1 }}
        exit={{ height: 0, opacity: 0 }}
        transition={{ duration: 0.3, ease: "easeInOut" }}
        className="overflow-hidden"
      >
        <div
          className={`relative flex items-center justify-between gap-3 px-4 py-3 sm:px-6 lg:px-8 ${
            isOffline
              ? "bg-red-950/80 border-b border-red-800/50"
              : "bg-emerald-950/80 border-b border-emerald-800/50"
          }`}
        >
          <div className="flex items-center gap-3 text-sm font-medium">
            {isOffline ? (
              <>
                <span className="flex h-7 w-7 items-center justify-center rounded-full bg-red-900/60 ring-1 ring-red-700/50">
                  <WifiOff className="h-4 w-4 text-red-400" />
                </span>
                <span className="text-red-300">
                  You are offline. Check your connection.
                </span>
              </>
            ) : (
              <>
                <span className="flex h-7 w-7 items-center justify-center rounded-full bg-emerald-900/60 ring-1 ring-emerald-700/50">
                  <Wifi className="h-4 w-4 text-emerald-400" />
                </span>
                <span className="text-emerald-300">
                  Back online! Reconnecting...
                </span>
              </>
            )}
          </div>

          <button
            type="button"
            onClick={() => setDismissed(true)}
            aria-label="Dismiss notification"
            className={`flex h-7 w-7 items-center justify-center rounded-full transition-colors ${
              isOffline
                ? "text-red-400 hover:bg-red-800/60 hover:text-red-300"
                : "text-emerald-400 hover:bg-emerald-800/60 hover:text-emerald-300"
            }`}
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      </motion.div>
    </AnimatePresence>
  );
}

