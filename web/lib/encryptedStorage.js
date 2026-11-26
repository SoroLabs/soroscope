const ENVELOPE_VERSION = 1;
const IV_LENGTH = 12;

function bytesToBase64(bytes) {
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

function base64ToBytes(value) {
  const binary = atob(value);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

/**
 * Creates an encrypted LocalStorage facade. The AES-GCM key exists only in
 * memory, so a new browser session cannot decrypt data from an older session.
 */
function createEncryptedStorage(storage, cryptoProvider) {
  if (!storage || !cryptoProvider?.subtle) {
    throw new Error('Encrypted storage requires LocalStorage and Web Crypto');
  }

  let sessionKey;

  async function getSessionKey() {
    if (!sessionKey) {
      sessionKey = cryptoProvider.subtle.generateKey(
        { name: 'AES-GCM', length: 256 },
        false,
        ['encrypt', 'decrypt'],
      );
    }
    return sessionKey;
  }

  return {
    async setItem(key, value) {
      const iv = cryptoProvider.getRandomValues(new Uint8Array(IV_LENGTH));
      const encrypted = await cryptoProvider.subtle.encrypt(
        { name: 'AES-GCM', iv },
        await getSessionKey(),
        new TextEncoder().encode(value),
      );

      storage.setItem(
        key,
        JSON.stringify({
          version: ENVELOPE_VERSION,
          iv: bytesToBase64(iv),
          ciphertext: bytesToBase64(new Uint8Array(encrypted)),
        }),
      );
    },

    async getItem(key) {
      const stored = storage.getItem(key);
      if (stored === null) {
        return null;
      }

      try {
        const envelope = JSON.parse(stored);
        if (
          envelope.version !== ENVELOPE_VERSION ||
          typeof envelope.iv !== 'string' ||
          typeof envelope.ciphertext !== 'string'
        ) {
          throw new Error('Invalid encrypted storage envelope');
        }

        const decrypted = await cryptoProvider.subtle.decrypt(
          { name: 'AES-GCM', iv: base64ToBytes(envelope.iv) },
          await getSessionKey(),
          base64ToBytes(envelope.ciphertext),
        );
        return new TextDecoder().decode(decrypted);
      } catch {
        storage.removeItem(key);
        return null;
      }
    },

    removeItem(key) {
      storage.removeItem(key);
    },
  };
}

/**
 * A plain (unencrypted) localStorage facade that mirrors the EncryptedStorage
 * interface. Used as a fallback when Web Crypto is unavailable (e.g. HTTP,
 * old Safari without HTTPS) so data is still persisted rather than silently
 * discarded. Callers continue to work correctly with the same async API.
 *
 * @param {Storage} storage
 * @returns {import('./encryptedStorage').EncryptedStorage}
 */
function createPlaintextFallbackStorage(storage) {
  return {
    async setItem(key, value) {
      storage.setItem(key, value);
    },
    async getItem(key) {
      return storage.getItem(key);
    },
    removeItem(key) {
      storage.removeItem(key);
    },
  };
}

let browserStorage;

function getEncryptedLocalStorage() {
  if (typeof window === 'undefined' || !window.localStorage) {
    return null;
  }

  if (!browserStorage) {
    if (!globalThis.crypto?.subtle) {
      // crypto.subtle is only available in secure contexts (HTTPS / localhost
      // served over HTTPS). In plain HTTP or environments where it's absent we
      // fall back to unencrypted localStorage and emit a one-time console
      // warning so the issue is visible to developers without silently
      // discarding every write (fixes #1293).
      console.warn(
        '[SoroScope] Web Crypto API (crypto.subtle) is unavailable in this context. ' +
          'Storage will fall back to unencrypted localStorage. ' +
          'Serve the app over HTTPS to enable AES-GCM encryption.',
      );
      browserStorage = createPlaintextFallbackStorage(window.localStorage);
    } else {
      browserStorage = createEncryptedStorage(window.localStorage, globalThis.crypto);
    }
  }
  return browserStorage;
}

module.exports = { createEncryptedStorage, createPlaintextFallbackStorage, getEncryptedLocalStorage };
