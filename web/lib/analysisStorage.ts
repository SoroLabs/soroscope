/**
 * Client-side storage utilities for persisting the latest WASM analysis result.
 * 
 * This module handles saving and restoring the most recent analysis result
 * to/from browser local storage, allowing the UI to preserve state across
 * page refreshes.
 */

import type { InvocationResult } from './sorobantypes';
import { getEncryptedLocalStorage } from './encryptedStorage';
import {
  sanitizePlainText,
  sanitizeMermaidDefinition,
  stripControlChars,
} from './security';

const LATEST_ANALYSIS_KEY = 'soroscope-latest-analysis';

const MAX_FUNCTION_NAME_LENGTH = 64;
const MAX_RESULT_FIELD_LENGTH = 1_000_000;

/**
 * Sanitize an untrusted object (e.g. restored from localStorage, which any
 * attacker can edit) into a render-safe InvocationResult. Immutable string
 * fields are length-capped and control-char-stripped; the mermaid definition —
 * the only field rendered through raw innerHTML downstream — is bounded so a
 * tampered graph cannot be used for a stored-XSS payload.
 */
export function sanitizeRestoredResult(parsed: unknown): InvocationResult | null {
  if (!parsed || typeof parsed !== 'object') return null;
  const result = parsed as Record<string, unknown>;

  if (typeof result.id !== 'string' || result.id.length === 0) return null;

  const functionName = sanitizePlainText(result.functionName, MAX_FUNCTION_NAME_LENGTH);
  if (!functionName) return null;

  return {
    ...result,
    id: stripControlChars(result.id).slice(0, 128),
    functionName,
    error: typeof result.error === 'string' ? sanitizePlainText(result.error, MAX_RESULT_FIELD_LENGTH) : result.error,
    errorType: typeof result.errorType === 'string' ? sanitizePlainText(result.errorType, 256) : result.errorType,
    callGraphMermaid:
      typeof result.callGraphMermaid === 'string'
        ? sanitizeMermaidDefinition(result.callGraphMermaid)
        : result.callGraphMermaid,
    inputs:
      result.inputs && typeof result.inputs === 'object'
        ? sanitizeRestoredInputs(result.inputs)
        : result.inputs,
  } as InvocationResult;
}

function sanitizeRestoredInputs(inputs: Record<string, unknown>): Record<string, unknown> {
  const sanitized: Record<string, unknown> = {};
  for (const [name, value] of Object.entries(inputs)) {
    const key = stripControlChars(name).slice(0, 128);
    if (typeof value === 'string') {
      sanitized[key] = sanitizePlainText(value, MAX_RESULT_FIELD_LENGTH);
    } else {
      sanitized[key] = value;
    }
  }
  return sanitized;
}

/**
 * Check if we're running in a browser environment.
 * Prevents SSR/hydration errors in Next.js.
 */
/**
 * Save the latest analysis result to local storage.
 * Only the most recent result is kept - new results overwrite old ones.
 * 
 * @param result - The analysis result to persist
 */
export async function saveLatestAnalysis(result: InvocationResult): Promise<void> {
  const storage = getEncryptedLocalStorage();
  if (!storage) {
    return;
  }

  try {
    // Serialize the result, excluding any non-serializable values
    const serialized = JSON.stringify(result);
    await storage.setItem(LATEST_ANALYSIS_KEY, serialized);
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
export async function loadLatestAnalysis(): Promise<InvocationResult | null> {
  const storage = getEncryptedLocalStorage();
  if (!storage) {
    return null;
  }

  try {
    const stored = await storage.getItem(LATEST_ANALYSIS_KEY);
    if (!stored) {
      return null;
    }

    const parsed = JSON.parse(stored) as InvocationResult;

    // Validate and sanitize the stored data — localStorage is user-editable, so
    // restored content is treated as untrusted before it is re-rendered.
    const sanitized = sanitizeRestoredResult(parsed);
    if (!sanitized) {
      console.warn('Invalid analysis result in storage, ignoring');
      return null;
    }

    return sanitized;
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
  const storage = getEncryptedLocalStorage();
  if (!storage) {
    return;
  }

  try {
    storage.removeItem(LATEST_ANALYSIS_KEY);
  } catch (error) {
    console.warn('Failed to clear latest analysis from local storage:', error);
  }
}
