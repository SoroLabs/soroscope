/**
 * Client-side encryption helpers for sensitive LocalStorage values
 * (custom Soroban RPC endpoints and secret keys).
 *
 * Uses the Web Crypto API with AES-GCM. A per-browser AES key is generated
 * once and persisted (as raw base64) under a dedicated LocalStorage entry so
 * that encrypted payloads can be decrypted across page reloads.
 */

const KEY_STORAGE_KEY = 'soroban.rpc.encryption.key';
const IV_BYTE_LENGTH = 12;
const ENCRYPTED_PREFIX = 'enc:v1:';

function getCrypto(): Crypto {
  if (typeof globalThis === 'undefined' || !globalThis.crypto?.subtle) {
    throw new Error('Web Crypto API (AES-GCM) is not available in this environment.');
  }
  return globalThis.crypto;
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function base64ToBytes(value: string): Uint8Array {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

async function getOrCreateKey(): Promise<CryptoKey> {
  const crypto = getCrypto();
  const stored = typeof localStorage !== 'undefined' ? localStorage.getItem(KEY_STORAGE_KEY) : null;

  if (stored) {
    const raw = base64ToBytes(stored);
    return crypto.subtle.importKey('raw', raw, { name: 'AES-GCM' }, false, ['encrypt', 'decrypt']);
  }

  const key = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, true, [
    'encrypt',
    'decrypt',
  ]);
  const exported = new Uint8Array(await crypto.subtle.exportKey('raw', key));
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(KEY_STORAGE_KEY, bytesToBase64(exported));
  }
  return key;
}

/**
 * Encrypt a plaintext string with AES-GCM. Returns a self-describing payload
 * (`enc:v1:<iv>:<ciphertext>`, both base64) safe to persist in LocalStorage.
 */
export async function encryptValue(plaintext: string): Promise<string> {
  const crypto = getCrypto();
  const key = await getOrCreateKey();
  const iv = crypto.getRandomValues(new Uint8Array(IV_BYTE_LENGTH));
  const encoded = new TextEncoder().encode(plaintext);
  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, encoded),
  );
  return `${ENCRYPTED_PREFIX}${bytesToBase64(iv)}:${bytesToBase64(ciphertext)}`;
}

/**
 * Decrypt a payload produced by {@link encryptValue}. Plaintext values that
 * were never encrypted are returned unchanged for backward compatibility.
 */
export async function decryptValue(payload: string): Promise<string> {
  if (!payload.startsWith(ENCRYPTED_PREFIX)) {
    return payload;
  }

  const crypto = getCrypto();
  const [ivPart, cipherPart] = payload.slice(ENCRYPTED_PREFIX.length).split(':');
  if (!ivPart || !cipherPart) {
    throw new Error('Malformed encrypted payload.');
  }

  const key = await getOrCreateKey();
  const iv = base64ToBytes(ivPart);
  const ciphertext = base64ToBytes(cipherPart);
  const plaintext = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, ciphertext);
  return new TextDecoder().decode(plaintext);
}

/**
 * Encrypt and persist a value under the given LocalStorage key.
 */
export async function setEncryptedItem(storageKey: string, value: string): Promise<void> {
  const encrypted = await encryptValue(value);
  localStorage.setItem(storageKey, encrypted);
}

/**
 * Read and decrypt a value from LocalStorage. Returns `null` when absent.
 */
export async function getEncryptedItem(storageKey: string): Promise<string | null> {
  const stored = localStorage.getItem(storageKey);
  if (stored === null) {
    return null;
  }
  return decryptValue(stored);
}

/**
 * Remove a single encrypted entry from LocalStorage.
 */
export function removeEncryptedItem(storageKey: string): void {
  localStorage.removeItem(storageKey);
}

/**
 * Clear all stored Soroban RPC endpoints (and the derived encryption key).
 * Backs the "Clear All Stored Endpoints" settings action.
 */
export function clearAllStoredEndpoints(): void {
  if (typeof localStorage === 'undefined') {
    return;
  }
  const keysToRemove: string[] = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const key = localStorage.key(i);
    if (key && (key.startsWith('soroban.rpc') || key.startsWith('soroban.endpoint'))) {
      keysToRemove.push(key);
    }
  }
  keysToRemove.forEach((key) => localStorage.removeItem(key));
  localStorage.removeItem(KEY_STORAGE_KEY);
}
