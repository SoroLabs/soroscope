/**
 * SoroScope Service Worker
 *
 * Provides offline support by:
 * 1. Caching the app shell (HTML, CSS, JS) so the layout and nav load offline.
 * 2. Caching WASM validation/disassembly worker scripts.
 * 3. Intercepting fetch requests and serving cached responses when offline.
 *
 * Analysis results and WASM file data are cached in IndexedDB by the client
 * (see lib/offlineStorage.ts), not by this service worker, because IndexedDB
 * is better suited for large structured data than the Cache API.
 */

const CACHE_VERSION = 'soroscope-v1';
const APP_SHELL_CACHE = `${CACHE_VERSION}-app-shell`;
const WASM_CACHE = `${CACHE_VERSION}-wasm`;

// Resources that make up the app shell — precached on install.
const APP_SHELL_URLS = [
  '/',
  '/_next/static/chunks/main.js',
  '/_next/static/chunks/polyfills.js',
];

// Routes that should be network-first (API calls, dynamic pages).
const NETWORK_FIRST_PATTERNS = [
  /\/api\//,
];

// Routes that should be cache-first (static assets, _next chunks).
const CACHE_FIRST_PATTERNS = [
  /\/_next\/static\//,
  /\/_next\/chunks\//,
  /\/wasm\//,
];

/**
 * Install: precache the app shell.
 */
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(APP_SHELL_CACHE).then((cache) => {
      // Use individual fetches so a single 404 doesn't reject the whole batch.
      return Promise.allSettled(
        APP_SHELL_URLS.map((url) => cache.add(url)),
      );
    }).then(() => self.skipWaiting()),
  );
});

/**
 * Activate: clean up old caches and claim clients.
 */
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) => {
      return Promise.all(
        keys
          .filter((key) => !key.startsWith(CACHE_VERSION))
          .map((key) => caches.delete(key)),
      );
    }).then(() => self.clients.claim()),
  );
});

/**
 * Fetch: serve from cache when offline, use network-first for API calls,
 * cache-first for static assets.
 */
self.addEventListener('fetch', (event) => {
  const { request } = event;

  // Only handle GET requests.
  if (request.method !== 'GET') return;

  const url = new URL(request.url);

  // Skip cross-origin requests.
  if (url.origin !== self.location.origin) return;

  // Skip Next.js HMR and dev-only endpoints.
  if (url.pathname.startsWith('/_next/webpack-hmr')) return;

  // Network-first for API routes (don't serve stale API data).
  if (NETWORK_FIRST_PATTERNS.some((pattern) => pattern.test(url.pathname))) {
    event.respondWith(
      fetch(request).catch(() => caches.match(request)),
    );
    return;
  }

  // Cache-first for static assets and WASM files.
  if (CACHE_FIRST_PATTERNS.some((pattern) => pattern.test(url.pathname))) {
    event.respondWith(
      caches.match(request).then((cached) => {
        if (cached) {
          // Update cache in background.
          fetch(request).then((resp) => {
            if (resp && resp.status === 200) {
              const cacheName = url.pathname.startsWith('/wasm/') ? WASM_CACHE : APP_SHELL_CACHE;
              const clone = resp.clone();
              caches.open(cacheName).then((cache) => cache.put(request, clone));
            }
          }).catch(() => {});
          return cached;
        }
        return fetch(request).then((resp) => {
          if (resp && resp.status === 200) {
            const cacheName = url.pathname.startsWith('/wasm/') ? WASM_CACHE : APP_SHELL_CACHE;
            const clone = resp.clone();
            caches.open(cacheName).then((cache) => cache.put(request, clone));
          }
          return resp;
        }).catch(() => new Response('Offline', { status: 503, statusText: 'Offline' }));
      }),
    );
    return;
  }

  // For page navigations: try network first, fall back to cached app shell.
  if (request.mode === 'navigate') {
    event.respondWith(
      fetch(request).catch(() => caches.match('/').then((cached) => cached || new Response(
        '<!DOCTYPE html><html><body><h1>Offline</h1><p>The app shell could not be loaded.</p></body></html>',
        { headers: { 'Content-Type': 'text/html' } },
      ))),
    );
    return;
  }
});

/**
 * Message handler: allows the page to trigger cache cleanup or updates.
 */
self.addEventListener('message', (event) => {
  if (event.data === 'SKIP_WAITING') {
    self.skipWaiting();
  }
  if (event.data === 'CLEAR_CACHES') {
    caches.keys().then((keys) => {
      return Promise.all(keys.map((key) => caches.delete(key)));
    });
  }
});
