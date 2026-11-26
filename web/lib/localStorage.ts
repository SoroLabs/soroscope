/**
 * LocalStorage helpers with AES-GCM encryption for sensitive Soroban RPC
 * endpoint configuration (custom RPC URLs and secret keys).
 *
 * Encrypted values are stored as JSON envelopes of the form:
 *   { __enc: true, iv: <base64>, data: <base64> }
 * so that plaintext values written by older versions remain readable.
 */

const ENCRYPTION_KEY_STORAGE = 'soroban_rpc_enc_key';
const RPC_ENDPOINTS_STORAGE = 'soroban_rpc_endpoints';
const RPC_SECRET_STORAGE = 'soroban_rpc_secret';

export const NETWORK_STORAGE_KEY = 'soroscope_selected_network';
export const STORAGE_KEY = NETWORK_STORAGE_KEY;

interface EncryptedEnvelope {
  __enc: true;
  iv: string;
  data: string;
}

function isEncryptedEnvelope(value: unknown): value is EncryptedEnvelope {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as { __enc?: unknown }).__enc === true &&
    typeof (value as { iv?: unknown }).iv === 'string' &&
    typeof (value as { data?: unknown }).data === 'string'
  );
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

async function getEncryptionKey(): Promise<CryptoKey> {
  const stored = localStorage.getItem(ENCRYPTION_KEY_STORAGE);
  if (stored) {
    const raw = base64ToBytes(stored);
    return crypto.subtle.importKey('raw', raw, { name: 'AES-GCM' }, false, [
      'encrypt',
      'decrypt',
    ]);
  }

  const key = await crypto.subtle.generateKey(
    { name: 'AES-GCM', length: 256 },
    true,
    ['encrypt', 'decrypt'],
  );
  const exported = new Uint8Array(await crypto.subtle.exportKey('raw', key));
  localStorage.setItem(ENCRYPTION_KEY_STORAGE, bytesToBase64(exported));
  return key;
}

async function encryptValue(plaintext: string): Promise<EncryptedEnvelope> {
  const key = await getEncryptionKey();
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const encoded = new TextEncoder().encode(plaintext);
  const cipher = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, encoded);
  return {
    __enc: true,
    iv: bytesToBase64(iv),
    data: bytesToBase64(new Uint8Array(cipher)),
  };
}

async function decryptValue(envelope: EncryptedEnvelope): Promise<string> {
  const key = await getEncryptionKey();
  const iv = base64ToBytes(envelope.iv);
  const data = base64ToBytes(envelope.data);
  const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, data);
  return new TextDecoder().decode(plain);
}

async function readEncrypted<T>(storageKey: string): Promise<T | null> {
  const raw = localStorage.getItem(storageKey);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    if (isEncryptedEnvelope(parsed)) {
      return JSON.parse(await decryptValue(parsed)) as T;
    }
    return parsed as T;
  } catch {
    return null;
  }
}

async function writeEncrypted(storageKey: string, value: unknown): Promise<void> {
  const envelope = await encryptValue(JSON.stringify(value));
  localStorage.setItem(storageKey, JSON.stringify(envelope));
}

/**
 * Persist the list of custom Soroban RPC endpoints, encrypting the payload
 * with AES-GCM before it touches LocalStorage.
 */
export async function saveRpcEndpoints(endpoints: string[]): Promise<void> {
  await writeEncrypted(RPC_ENDPOINTS_STORAGE, endpoints);
}

/**
 * Read and decrypt the stored custom Soroban RPC endpoints.
 */
export async function getRpcEndpoints(): Promise<string[]> {
  const endpoints = await readEncrypted<string[]>(RPC_ENDPOINTS_STORAGE);
  return Array.isArray(endpoints) ? endpoints : [];
}

/**
 * Persist a Soroban secret key, encrypting it with AES-GCM before storage.
 */
export async function saveRpcSecret(secret: string): Promise<void> {
  await writeEncrypted(RPC_SECRET_STORAGE, secret);
}

/**
 * Read and decrypt the stored Soroban secret key.
 */
export async function getRpcSecret(): Promise<string | null> {
  return readEncrypted<string>(RPC_SECRET_STORAGE);
}

/**
 * Remove every stored Soroban RPC endpoint and secret from LocalStorage.
 * Backs the "Clear All Stored Endpoints" settings action.
 */
export function clearAllStoredEndpoints(): void {
  localStorage.removeItem(RPC_ENDPOINTS_STORAGE);
  localStorage.removeItem(RPC_SECRET_STORAGE);
}

function getStorage(): Storage | null {
  if (typeof window !== 'undefined' && window.localStorage) {
    return window.localStorage;
  }
  if (typeof localStorage !== 'undefined') {
    return localStorage;
  }
  return null;
}

/**
 * Persist the selected Stellar network ID to LocalStorage.
 */
export function saveSelectedNetwork(networkId: string): void {
  try {
    const storage = getStorage();
    if (storage) {
      storage.setItem(NETWORK_STORAGE_KEY, networkId);
    }
  } catch (e) {
    console.warn('Failed to persist selected network preference', e);
  }
}

/**
 * Read the persisted Stellar network ID from LocalStorage.
 */
export function getSelectedNetwork(): string | null {
  try {
    const storage = getStorage();
    if (storage) {
      return storage.getItem(NETWORK_STORAGE_KEY);
    }
  } catch (e) {
    console.warn('Failed to load selected network preference', e);
  }
  return null;
}

/**
 * Clear the selected Stellar network ID from LocalStorage.
 */
export function clearSelectedNetwork(): void {
  try {
    const storage = getStorage();
    if (storage) {
      storage.removeItem(NETWORK_STORAGE_KEY);
    }
  } catch (e) {
    console.warn('Failed to clear selected network preference', e);
  }
}

/* -------------------------------------------------------------------------- */
/* Shareable permalink state codec                                            */
/* -------------------------------------------------------------------------- */

export const PERMALINK_STATE_PARAM = 'state';

export interface SimulationState {
  network: string;
  contractId: string;
  fnName: string;
  args: string[];
  viewTab: string;
}

export const DEFAULT_SIMULATION_STATE: SimulationState = {
  network: 'testnet',
  contractId: '',
  fnName: '',
  args: [],
  viewTab: 'simulate',
};

const MAX_STATE_LENGTH = 8192;
const MAX_ARGS = 50;
const MAX_FIELD_LENGTH = 4096;

function bytesToBase64Url(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function base64UrlToBytes(base64url: string): Uint8Array {
  const normalized = base64url.replace(/-/g, '+').replace(/_/g, '/');
  const padded = normalized + '='.repeat((4 - (normalized.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

function isSafeString(value: unknown, maxLength = MAX_FIELD_LENGTH): value is string {
  return typeof value === 'string' && value.length <= maxLength;
}

/**
 * Encode a simulation state into a compact base64url string suitable for a
 * `?state=` query parameter. Returns an empty string when the state cannot be
 * serialized so callers can fall back gracefully.
 */
export function encodeSimulationState(state: SimulationState): string {
  try {
    const payload = {
      n: state.network,
      c: state.contractId,
      f: state.fnName,
      a: state.args,
      t: state.viewTab,
    };
    const json = JSON.stringify(payload);
    return bytesToBase64Url(new TextEncoder().encode(json));
  } catch {
    return '';
  }
}

/**
 * Decode a `?state=` query parameter back into a validated simulation state.
 * Malformed, oversized, or tampered input falls back to the default state.
 */
export function decodeSimulationState(encoded: string | null | undefined): SimulationState {
  if (!encoded || typeof encoded !== 'string' || encoded.length > MAX_STATE_LENGTH) {
    return { ...DEFAULT_SIMULATION_STATE };
  }

  try {
    const json = new TextDecoder().decode(base64UrlToBytes(encoded));
    const parsed = JSON.parse(json) as Record<string, unknown>;
    if (typeof parsed !== 'object' || parsed === null) {
      return { ...DEFAULT_SIMULATION_STATE };
    }

    const network = isSafeString(parsed.n) ? parsed.n : DEFAULT_SIMULATION_STATE.network;
    const contractId = isSafeString(parsed.c) ? parsed.c : DEFAULT_SIMULATION_STATE.contractId;
    const fnName = isSafeString(parsed.f) ? parsed.f : DEFAULT_SIMULATION_STATE.fnName;
    const viewTab = isSafeString(parsed.t) ? parsed.t : DEFAULT_SIMULATION_STATE.viewTab;

    let args: string[] = [];
    if (Array.isArray(parsed.a)) {
      args = parsed.a
        .filter((arg): arg is string => isSafeString(arg))
        .slice(0, MAX_ARGS);
    }

    return { network, contractId, fnName, args, viewTab };
  } catch {
    return { ...DEFAULT_SIMULATION_STATE };
  }
}

/**
 * Build a shareable permalink URL for the given simulation state.
 */
export function buildPermalink(
  state: SimulationState,
  baseUrl?: string,
): string {
  const encoded = encodeSimulationState(state);
  const base =
    baseUrl ??
    (typeof window !== 'undefined' && window.location
      ? `${window.location.origin}${window.location.pathname}`
      : '');
  if (!encoded) return base;
  return `${base}?${PERMALINK_STATE_PARAM}=${encoded}`;
}

/**
 * Read and decode the simulation state from a URL search string.
 */
export function readSimulationStateFromSearch(search: string): SimulationState {
  try {
    const params = new URLSearchParams(search);
    return decodeSimulationState(params.get(PERMALINK_STATE_PARAM));
  } catch {
    return { ...DEFAULT_SIMULATION_STATE };
  }
}
