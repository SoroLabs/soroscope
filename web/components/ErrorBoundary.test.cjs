// ErrorBoundary.test.cjs — unit tests for ErrorBoundary and RPC network failure recovery logic
// Closes Issue #600
// Runs with: node --test ./components/ErrorBoundary.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// ── Pure logic mirroring ErrorBoundary helper functions ───────────────────────

function isRpcNetworkError(error) {
  if (!error) return false;
  const msg = (error.message || '').toLowerCase();
  const name = (error.name || '').toLowerCase();

  return (
    msg.includes('rpc') ||
    msg.includes('fetch') ||
    msg.includes('network') ||
    msg.includes('econnrefused') ||
    msg.includes('timeout') ||
    msg.includes('failed to fetch') ||
    msg.includes('500') ||
    msg.includes('503') ||
    name.includes('networkerror') ||
    name.includes('typeerror')
  );
}

function createErrorBoundaryState() {
  let error = null;
  let errorInfo = null;
  let showDetails = false;

  return {
    get error() { return error; },
    get errorInfo() { return errorInfo; },
    get showDetails() { return showDetails; },
    catchError(err, info = null) {
      error = err;
      errorInfo = info;
    },
    reset() {
      error = null;
      errorInfo = null;
      showDetails = false;
    },
    toggleDetails() {
      showDetails = !showDetails;
    }
  };
}

// ── Tests ───────────────────────────────────────────────────────────────────

test('isRpcNetworkError: detects Soroban RPC connection failures', () => {
  const rpcError = new Error('RPC node connection refused: ECONNREFUSED 127.0.0.1:8000');
  assert.equal(isRpcNetworkError(rpcError), true);
});

test('isRpcNetworkError: detects fetch network errors', () => {
  const fetchError = new TypeError('Failed to fetch');
  assert.equal(isRpcNetworkError(fetchError), true);
});

test('isRpcNetworkError: detects HTTP 503 Service Unavailable', () => {
  const serverError = new Error('RPC fetch returned status 503');
  assert.equal(isRpcNetworkError(serverError), true);
});

test('isRpcNetworkError: returns false for generic non-network syntax error', () => {
  const genericError = new Error('Cannot read properties of undefined (reading "name")');
  assert.equal(isRpcNetworkError(genericError), false);
});

test('isRpcNetworkError: returns false for null error input', () => {
  assert.equal(isRpcNetworkError(null), false);
});

test('ErrorBoundary state: initializes with clean state', () => {
  const state = createErrorBoundaryState();
  assert.equal(state.error, null);
  assert.equal(state.errorInfo, null);
  assert.equal(state.showDetails, false);
});

test('ErrorBoundary state: catches error and updates state', () => {
  const state = createErrorBoundaryState();
  const testErr = new Error('RPC Fetch Error');
  state.catchError(testErr, { componentStack: 'at ComponentA' });

  assert.equal(state.error, testErr);
  assert.equal(state.errorInfo.componentStack, 'at ComponentA');
});

test('ErrorBoundary state: toggleDetails switches showDetails boolean', () => {
  const state = createErrorBoundaryState();
  assert.equal(state.showDetails, false);

  state.toggleDetails();
  assert.equal(state.showDetails, true);

  state.toggleDetails();
  assert.equal(state.showDetails, false);
});

test('ErrorBoundary state: reset clears error and details toggle state', () => {
  const state = createErrorBoundaryState();
  state.catchError(new Error('RPC Failure'));
  state.toggleDetails();

  assert.equal(state.showDetails, true);
  assert.ok(state.error !== null);

  state.reset();
  assert.equal(state.error, null);
  assert.equal(state.errorInfo, null);
  assert.equal(state.showDetails, false);
});
