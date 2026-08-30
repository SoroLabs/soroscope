/**
 * Client-side storage utilities for persisting the latest WASM analysis result.
 * 
 * This module handles saving and restoring the most recent analysis result
 * to/from browser local storage, allowing the UI to preserve state across
 * page refreshes.
 */

import type { InvocationResult } from './sorobantypes';

const LATEST_ANALYSIS_KEY = 'soroscope-latest-analysis';

/**
 * Check if we're running in a browser environment.
 * Prevents SSR/hydration errors in Next.js.
 */
function isBrowser(): boolean {
  return typeof window !== 'undefined' && typeof window.localStorage !== 'undefined';
}

/**
 * Save the latest analysis result to local storage.
 * Only the most recent result is kept - new results overwrite old ones.
 * 
 * @param result - The analysis result to persist
 */
export function saveLatestAnalysis(result: InvocationResult): void {
  if (!isBrowser()) {
    return;
  }

  try {
    // Serialize the result, excluding any non-serializable values
    const serialized = JSON.stringify(result);
    localStorage.setItem(LATEST_ANALYSIS_KEY, serialized);
  } catch (error) {
    // Silently fail if storage is full or unavailable
    console.warn('Failed to save latest analysis to local storage:', error);
  }
}

/**
 * Restore the latest analysis result from local storage.
 * Returns null if no valid result is found or if parsing fails.
 * 
 * @returns The restored analysis result, or null if unavailable
 */
export function loadLatestAnalysis(): InvocationResult | null {
  if (!isBrowser()) {
    return null;
  }

  try {
    const stored = localStorage.getItem(LATEST_ANALYSIS_KEY);
    if (!stored) {
      return null;
    }

    const parsed = JSON.parse(stored) as InvocationResult;
    
    // Basic validation to ensure the stored data has the expected shape
    if (!parsed || typeof parsed !== 'object' || !parsed.id || !parsed.functionName) {
      console.warn('Invalid analysis result in storage, ignoring');
      return null;
    }

    return parsed;
  } catch (error) {
    // Handle malformed JSON or other parsing errors gracefully
    console.warn('Failed to load latest analysis from local storage:', error);
    return null;
  }
}

/**
 * Clear the stored latest analysis result.
 * Useful for cleanup or when explicitly resetting the UI state.
 */
export function clearLatestAnalysis(): void {
  if (!isBrowser()) {
    return;
  }

  try {
    localStorage.removeItem(LATEST_ANALYSIS_KEY);
  } catch (error) {
    console.warn('Failed to clear latest analysis from local storage:', error);
  }
}
