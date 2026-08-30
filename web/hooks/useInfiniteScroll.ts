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
          if (firstEntry && firstEntry.isIntersecting && hasMore && !isLoading) {
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
