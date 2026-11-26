/**
 * Close-code policy for `useWebSocket`.
 *
 * Kept outside the hook so it can be unit tested with `node --test`.
 *
 * Only transient failures are retried. Anything else (normal close, policy
 * violation, auth failure, ...) would fail again the same way, so retrying it
 * only floods the server with rejected connections.
 */

/** 1001 = endpoint going away, 1006 = abnormal close (no close frame). */
const RECONNECTABLE_CLOSE_CODES = Object.freeze([1001, 1006]);

/** Application close code the server sends when the client is not authorised. */
const AUTH_FAILURE_CLOSE_CODE = 4003;

function shouldReconnect(code) {
  return RECONNECTABLE_CLOSE_CODES.includes(code);
}

function isAuthFailure(code) {
  return code === AUTH_FAILURE_CLOSE_CODE;
}

module.exports = {
  AUTH_FAILURE_CLOSE_CODE,
  RECONNECTABLE_CLOSE_CODES,
  isAuthFailure,
  shouldReconnect,
};
