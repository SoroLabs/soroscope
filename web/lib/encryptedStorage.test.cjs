const test = require('node:test');
const assert = require('node:assert/strict');
const { webcrypto } = require('node:crypto');
const { createEncryptedStorage } = require('./encryptedStorage');

function memoryStorage() {
  const values = new Map();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: (key) => values.delete(key),
  };
}

test('encrypts values at rest and decrypts them in the same session', async () => {
  const storage = memoryStorage();
  const encryptedStorage = createEncryptedStorage(storage, webcrypto);

  await encryptedStorage.setItem('session', '{"wallet":"GSECRET"}');

  const persisted = storage.getItem('session');
  assert.equal(persisted.includes('GSECRET'), false);
  assert.equal(await encryptedStorage.getItem('session'), '{"wallet":"GSECRET"}');
});

test('uses a unique IV when the same value is stored twice', async () => {
  const storage = memoryStorage();
  const encryptedStorage = createEncryptedStorage(storage, webcrypto);

  await encryptedStorage.setItem('session', 'sensitive');
  const first = storage.getItem('session');
  await encryptedStorage.setItem('session', 'sensitive');

  assert.notEqual(storage.getItem('session'), first);
});

test('removes data that cannot be authenticated or decrypted', async () => {
  const storage = memoryStorage();
  const firstSession = createEncryptedStorage(storage, webcrypto);
  await firstSession.setItem('session', 'sensitive');

  const nextSession = createEncryptedStorage(storage, webcrypto);
  assert.equal(await nextSession.getItem('session'), null);
  assert.equal(storage.getItem('session'), null);
});

test('removes encrypted values', async () => {
  const storage = memoryStorage();
  const encryptedStorage = createEncryptedStorage(storage, webcrypto);
  await encryptedStorage.setItem('session', 'sensitive');

  encryptedStorage.removeItem('session');

  assert.equal(storage.getItem('session'), null);
});
