import { useCallback, useEffect, useRef } from 'react';

export interface UseInfiniteScrollOptions {
  /** Callback fired when the target sentinel element scrolls into view. */
  onLoadMore: () => void | Promise<void>;
  /** Whether there are more items to fetch/load. */
  hasMore: boolean;
  /** Whether a load operation is currently in progress. */
  isLoading?: boolean;
  /** IntersectionObserver threshold option (0.0 to 1.0). Defaults to 0.1. */
  threshold?: number;
  /** IntersectionObserver rootMargin option. Defaults to '100px'. */
  rootMargin?: string;
}

/**
 * React hook using IntersectionObserver to implement smooth infinite scroll pagination
 * for historical event logs and telemetry records.
 *
 * The `inFlightRef` guard ensures that when the observer fires multiple times
 * in rapid succession (e.g. fast scroll before the first fetch resolves),
 * only one `onLoadMore` call is in-flight at a time. Without this guard,
 * `isLoading` is still `false` for the second observer entry that fires before
 * the first async fetch updates React state, causing duplicate API requests
 * and duplicated rendered items.
 */
export function useInfiniteScroll({
  onLoadMore,
  hasMore,
  isLoading = false,
  threshold = 0.1,
  rootMargin = '100px',
}: UseInfiniteScrollOptions) {
  const observerRef = useRef<IntersectionObserver | null>(null);
  const targetNodeRef = useRef<HTMLElement | null>(null);
  // Tracks whether a fetch is already in-flight within this hook invocation,
  // guarding against duplicate calls triggered before React re-renders with
  // the updated `isLoading` state.
  const inFlightRef = useRef(false);

  // Reset the in-flight flag whenever the caller's isLoading transitions back
  // to false, meaning the previous fetch has completed.
  useEffect(() => {
    if (!isLoading) {
      inFlightRef.current = false;
    }
  }, [isLoading]);

  const setTargetRef = useCallback(
    (node: HTMLElement | null) => {
      targetNodeRef.current = node;

      if (observerRef.current) {
        observerRef.current.disconnect();
        observerRef.current = null;
      }

      if (!node || !hasMore || isLoading) {
        return;
      }

      if (typeof window === 'undefined' || !('IntersectionObserver' in window)) {
        return;
      }

      observerRef.current = new IntersectionObserver(
        (entries) => {
          const firstEntry = entries[0];
          if (
            firstEntry &&
            firstEntry.isIntersecting &&
            hasMore &&
            !isLoading &&
            !inFlightRef.current
          ) {
            inFlightRef.current = true;
            void onLoadMore();
          }
        },
        { threshold, rootMargin },
      );

      observerRef.current.observe(node);
    },
    [onLoadMore, hasMore, isLoading, threshold, rootMargin],
  );

  useEffect(() => {
    return () => {
      if (observerRef.current) {
        observerRef.current.disconnect();
        observerRef.current = null;
      }
    };
  }, []);

  return setTargetRef;
}
