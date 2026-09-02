import { useCallback, useEffect, useRef, useState } from 'react';

// ─── Types ────────────────────────────────────────────────────────────────────

/** Possible states of the WebSocket connection lifecycle. */
export type WebSocketStatus = 'connecting' | 'open' | 'closing' | 'closed' | 'error';

/** Options accepted by the `useWebSocket` hook. */
export interface UseWebSocketOptions<T = unknown> {
  /**
   * WebSocket server URL.
   * Pass `null` or `undefined` to skip connecting (useful for conditional usage).
   */
  url: string | null | undefined;
  /**
   * Called whenever a message is received from the server.
   * The raw `MessageEvent` is passed along with the parsed `data` payload.
   */
  onMessage?: (event: MessageEvent, data: T) => void;
  /**
   * Called when the connection is established.
   */
  onOpen?: (event: Event) => void;
  /**
   * Called when the connection is closed.
   */
  onClose?: (event: CloseEvent) => void;
  /**
   * Called when an error event is emitted.
   */
  onError?: (event: Event) => void;
  /**
   * Optional JSON reviver for `JSON.parse`.
   * Defaults to no reviver (raw parse).
   */
  reviver?: Parameters<typeof JSON.parse>[1];
  /**
   * Sub-protocols forwarded to the `WebSocket` constructor.
   */
  protocols?: string | string[];
  /**
   * Whether to reconnect automatically on unexpected close.
   * Defaults to `false`.
   */
  reconnect?: boolean;
  /**
   * Base delay in milliseconds between reconnection attempts.
   * Defaults to `1000` ms.
   */
  reconnectDelay?: number;
  /**
   * Maximum number of reconnection attempts before giving up.
   * Defaults to `5`.
   */
  maxReconnectAttempts?: number;
}

/** Value returned by the `useWebSocket` hook. */
export interface UseWebSocketReturn<T = unknown> {
  /** Current connection status. */
  status: WebSocketStatus;
  /** Latest parsed message received from the server, or `null` if none yet. */
  lastMessage: T | null;
  /**
   * Send a message to the server.
   * Strings are sent as-is; all other values are JSON-serialised.
   * Returns `true` if the message was enqueued, `false` otherwise.
   */
  send: (data: unknown) => boolean;
  /**
   * Manually close the WebSocket.
   * Sets an internal flag to suppress automatic reconnection.
   */
  close: () => void;
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

/**
 * React hook that manages a WebSocket connection with proper cleanup.
 *
 * Fixes memory leaks caused by dangling event listeners and unclosed sockets
 * when the consuming component unmounts (Issue #133).
 *
 * Key guarantees:
 * - All event listeners (`onopen`, `onmessage`, `onerror`, `onclose`) are
 *   removed and the socket is closed when the component unmounts or when
 *   `url` changes.
 * - Only one active connection exists at a time — previous sockets are torn
 *   down before a new one is opened.
 * - Optional exponential-back-off reconnection is bounded by
 *   `maxReconnectAttempts`.
 *
 * @example
 * ```tsx
 * const { status, lastMessage, send } = useWebSocket<{ price: number }>({
 *   url: 'wss://stream.example.com/prices',
 *   onMessage: (_evt, data) => console.log(data.price),
 * });
 * ```
 */
export function useWebSocket<T = unknown>({
  url,
  onMessage,
  onOpen,
  onClose,
  onError,
  reviver,
  protocols,
  reconnect = false,
  reconnectDelay = 1000,
  maxReconnectAttempts = 5,
}: UseWebSocketOptions<T>): UseWebSocketReturn<T> {
  const [status, setStatus] = useState<WebSocketStatus>('closed');
  const [lastMessage, setLastMessage] = useState<T | null>(null);

  // Stable callback refs — updated on every render so handlers always have
  // access to the latest props without needing to be listed as useEffect deps.
  const onMessageRef = useRef(onMessage);
  const onOpenRef = useRef(onOpen);
  const onCloseRef = useRef(onClose);
  const onErrorRef = useRef(onError);
  const reviverRef = useRef(reviver);

  useEffect(() => {
    onMessageRef.current = onMessage;
  }, [onMessage]);

  useEffect(() => {
    onOpenRef.current = onOpen;
  }, [onOpen]);

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    onErrorRef.current = onError;
  }, [onError]);

  useEffect(() => {
    reviverRef.current = reviver;
  }, [reviver]);

  // Mutable refs that survive re-renders without triggering them.
  const socketRef = useRef<WebSocket | null>(null);
  const reconnectAttempts = useRef(0);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** Set to `true` when the caller explicitly calls `close()`. */
  const intentionalClose = useRef(false);

  // ── Core connection logic ────────────────────────────────────────────────────

  const connect = useCallback(
    (wsUrl: string) => {
      // Tear down any previous socket before opening a new one.
      if (socketRef.current) {
        const prev = socketRef.current;
        prev.onopen = null;
        prev.onmessage = null;
        prev.onerror = null;
        prev.onclose = null;
        prev.close();
        socketRef.current = null;
      }

      setStatus('connecting');

      let ws: WebSocket;
      try {
        ws = new WebSocket(wsUrl, protocols);
      } catch {
        setStatus('error');
        return;
      }

      socketRef.current = ws;

      ws.onopen = (event: Event) => {
        setStatus('open');
        reconnectAttempts.current = 0;
        onOpenRef.current?.(event);
      };

      ws.onmessage = (event: MessageEvent) => {
        let parsed: T;
        try {
          parsed = JSON.parse(event.data as string, reviverRef.current) as T;
        } catch {
          // Non-JSON frames are surfaced as raw strings.
          parsed = event.data as unknown as T;
        }
        setLastMessage(parsed);
        onMessageRef.current?.(event, parsed);
      };

      ws.onerror = (event: Event) => {
        setStatus('error');
        onErrorRef.current?.(event);
      };

      ws.onclose = (event: CloseEvent) => {
        setStatus('closed');
        socketRef.current = null;
        onCloseRef.current?.(event);

        // Attempt reconnection only when the close was unexpected and the
        // consumer has not explicitly requested a teardown.
        if (
          reconnect &&
          !intentionalClose.current &&
          reconnectAttempts.current < maxReconnectAttempts
        ) {
          const delay = reconnectDelay * Math.pow(2, reconnectAttempts.current);
          reconnectAttempts.current += 1;
          reconnectTimerRef.current = setTimeout(() => {
            connect(wsUrl);
          }, delay);
        }
      };
    },
    // `protocols`, `reconnect`, `reconnectDelay`, and `maxReconnectAttempts`
    // are intentionally stable primitives; `connect` itself is stable as long
    // as those do not change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [protocols, reconnect, reconnectDelay, maxReconnectAttempts],
  );

  // ── Effect: open / close socket when url changes ─────────────────────────────

  useEffect(() => {
    if (!url) {
      // URL not provided — ensure any existing connection is cleaned up.
      intentionalClose.current = true;
      if (socketRef.current) {
        const s = socketRef.current;
        s.onopen = null;
        s.onmessage = null;
        s.onerror = null;
        s.onclose = null;
        s.close();
        socketRef.current = null;
      }
      setStatus('closed');
      return;
    }

    intentionalClose.current = false;
    reconnectAttempts.current = 0;
    connect(url);

    /**
     * Cleanup function — runs on component unmount *and* before the effect
     * re-runs (i.e., when `url` changes). This is the critical fix for the
     * memory leak described in Issue #133:
     *
     * Without this cleanup the previous WebSocket's event listeners would
     * keep the component instance alive in memory and fire callbacks on a
     * component that is no longer mounted, potentially duplicating message
     * handlers with every remount.
     */
    return () => {
      // Suppress reconnection triggered by the intentional close below.
      intentionalClose.current = true;

      // Cancel any pending reconnect timer.
      if (reconnectTimerRef.current !== null) {
        clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }

      // Remove all listeners and close the socket.
      if (socketRef.current) {
        const s = socketRef.current;
        s.onopen = null;
        s.onmessage = null;
        s.onerror = null;
        s.onclose = null;
        s.close();
        socketRef.current = null;
      }
    };
  }, [url, connect]);

  // ── Public API ───────────────────────────────────────────────────────────────

  const send = useCallback((data: unknown): boolean => {
    const ws = socketRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      return false;
    }
    const payload = typeof data === 'string' ? data : JSON.stringify(data);
    ws.send(payload);
    return true;
  }, []);

  const close = useCallback(() => {
    intentionalClose.current = true;

    if (reconnectTimerRef.current !== null) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }

    if (socketRef.current) {
      setStatus('closing');
      const s = socketRef.current;
      s.onopen = null;
      s.onmessage = null;
      s.onerror = null;
      // Keep onclose temporarily so the status transitions to 'closed'.
      s.onclose = () => {
        setStatus('closed');
        socketRef.current = null;
      };
      s.close();
    }
  }, []);

  return { status, lastMessage, send, close };
}
