/**
 * App-wide toast channel.
 *
 * Code outside the React tree (hooks, API helpers) calls `emitToast()`; the
 * `<ToastHost />` mounted in `_app.tsx` listens for the event and renders it.
 */

const TOAST_EVENT = 'soroscope:toast';

const TOAST_TYPES = ['error', 'success', 'warning', 'info'];

let counter = 0;

/** Builds a toast with a unique id. Unknown types fall back to `info`. */
function createToast({ message, type = 'info' }) {
  counter += 1;
  return {
    id: `toast-${Date.now()}-${counter}`,
    message: String(message),
    type: TOAST_TYPES.includes(type) ? type : 'info',
  };
}

/** Dispatches a toast. Returns `false` when there is no `window` (SSR). */
function emitToast(toast, target = typeof window !== 'undefined' ? window : undefined) {
  if (!target || typeof target.dispatchEvent !== 'function') return false;
  target.dispatchEvent(new CustomEvent(TOAST_EVENT, { detail: createToast(toast) }));
  return true;
}

module.exports = {
  TOAST_EVENT,
  createToast,
  emitToast,
};
