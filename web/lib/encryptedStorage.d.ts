export interface EncryptedStorage {
  setItem(key: string, value: string): Promise<void>;
  getItem(key: string): Promise<string | null>;
  removeItem(key: string): void;
}

export function createEncryptedStorage(storage: Storage, cryptoProvider: Crypto): EncryptedStorage;
export function getEncryptedLocalStorage(): EncryptedStorage | null;
