/**
 * Offline analysis storage using IndexedDB.
 *
 * Caches WASM file data and analysis results so they remain available when
 * the user is offline. The service worker handles app-shell caching; this
 * module handles structured data that is too large or too dynamic for the
 * Cache API.
 */

import type { InvocationResult } from './sorobantypes';

const DB_NAME = 'soroscope-offline';
const DB_VERSION = 1;
const ANALYSIS_STORE = 'analyses';
const WASM_STORE = 'wasm-files';

export interface CachedAnalysis {
  id: string;
  functionName: string;
  timestamp: number;
  result: InvocationResult;
  wasmFileName?: string;
}

export interface CachedWasmFile {
  id: string;
  name: string;
  size: number;
  data: string; // base64-encoded
  timestamp: number;
}

let dbPromise: Promise<IDBDatabase> | null = null;

/**
 * Open (or create) the IndexedDB database.
 */
function openDB(): Promise<IDBDatabase> {
  if (dbPromise) return dbPromise;

  dbPromise = new Promise((resolve, reject) => {
    if (typeof indexedDB === 'undefined') {
      reject(new Error('IndexedDB is not available'));
      return;
    }

    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onupgradeneeded = () => {
      const db = request.result;

      if (!db.objectStoreNames.contains(ANALYSIS_STORE)) {
        db.createObjectStore(ANALYSIS_STORE, { keyPath: 'id' });
      }

      if (!db.objectStoreNames.contains(WASM_STORE)) {
        db.createObjectStore(WASM_STORE, { keyPath: 'id' });
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });

  return dbPromise;
}

/**
 * Save an analysis result to IndexedDB for offline access.
 */
export async function cacheAnalysis(
  result: InvocationResult,
  wasmFileName?: string,
): Promise<void> {
  try {
    const db = await openDB();
    const tx = db.transaction(ANALYSIS_STORE, 'readwrite');
    const store = tx.objectStore(ANALYSIS_STORE);

    const entry: CachedAnalysis = {
      id: result.id,
      functionName: result.functionName,
      timestamp: result.timestamp,
      result,
      wasmFileName,
    };

    await new Promise<void>((resolve, reject) => {
      const req = store.put(entry);
      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error);
    });

    tx.commit();
  } catch (error) {
    console.warn('Failed to cache analysis for offline use:', error);
  }
}

/**
 * Retrieve all cached analyses from IndexedDB.
 */
export async function getCachedAnalyses(): Promise<CachedAnalysis[]> {
  try {
    const db = await openDB();
    const tx = db.transaction(ANALYSIS_STORE, 'readonly');
    const store = tx.objectStore(ANALYSIS_STORE);

    return new Promise<CachedAnalysis[]>((resolve, reject) => {
      const req = store.getAll();
      req.onsuccess = () => {
        const results = (req.result || []) as CachedAnalysis[];
        // Sort newest-first.
        results.sort((a, b) => b.timestamp - a.timestamp);
        resolve(results);
      };
      req.onerror = () => reject(req.error);
    });
  } catch (error) {
    console.warn('Failed to retrieve cached analyses:', error);
    return [];
  }
}

/**
 * Delete a cached analysis by ID.
 */
export async function deleteCachedAnalysis(id: string): Promise<void> {
  try {
    const db = await openDB();
    const tx = db.transaction(ANALYSIS_STORE, 'readwrite');
    await new Promise<void>((resolve, reject) => {
      const req = tx.objectStore(ANALYSIS_STORE).delete(id);
      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error);
    });
  } catch (error) {
    console.warn('Failed to delete cached analysis:', error);
  }
}

/**
 * Save a WASM file to IndexedDB for offline access.
 */
export async function cacheWasmFile(
  id: string,
  name: string,
  data: string,
): Promise<void> {
  try {
    const db = await openDB();
    const tx = db.transaction(WASM_STORE, 'readwrite');
    const store = tx.objectStore(WASM_STORE);

    const entry: CachedWasmFile = {
      id,
      name,
      size: data.length,
      data,
      timestamp: Date.now(),
    };

    await new Promise<void>((resolve, reject) => {
      const req = store.put(entry);
      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error);
    });

    tx.commit();
  } catch (error) {
    console.warn('Failed to cache WASM file for offline use:', error);
  }
}

/**
 * Retrieve all cached WASM files from IndexedDB.
 */
export async function getCachedWasmFiles(): Promise<CachedWasmFile[]> {
  try {
    const db = await openDB();
    const tx = db.transaction(WASM_STORE, 'readonly');
    const store = tx.objectStore(WASM_STORE);

    return new Promise<CachedWasmFile[]>((resolve, reject) => {
      const req = store.getAll();
      req.onsuccess = () => {
        const results = (req.result || []) as CachedWasmFile[];
        results.sort((a, b) => b.timestamp - a.timestamp);
        resolve(results);
      };
      req.onerror = () => reject(req.error);
    });
  } catch (error)    {
    console.warn('Failed to retrieve cached WASM files:', error);
    return [];
  }
}

/**
 * Check whether IndexedDB is available in the current environment.
 */
export function isOfflineStorageAvailable(): boolean {
  return typeof indexedDB !== 'undefined';
}
