// ErrorBoundary.test.cjs — unit tests for ErrorBoundary and RPC network failure recovery logic
// Closes Issue #600
// Runs with: node --test ./components/ErrorBoundary.test.cjs

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// ── Pure logic mirroring ErrorBoundary helper functions ───────────────────────

function sanitizeTelemetry(text) {
  if (!text) return '';
  return text
    .replace(/\bS[A-Z2-7]{55}\b/g, '[REDACTED_STELLAR_SECRET_KEY]')
    .replace(/Bearer\s+[A-Za-z0-9\-_.~+/]+=*/gi, 'Bearer [REDACTED_BEARER_TOKEN]')
    .replace(/(api[_-]?key|secret|token|password|auth|authorization)=["']?[^&"'\s]+/gi, '$1=[REDACTED]')
    .replace(/\b(0x)?[0-9a-fA-F]{64}\b/g, '[REDACTED_HEX_KEY]');
}

function buildDiagnosticsReport(error, errorInfo) {
  const lines = [
    `Error: ${error.name || 'Error'}`,
    `Message: ${sanitizeTelemetry(error.message || '(no message)')}`,
    '',
    'Stack Trace:',
    sanitizeTelemetry(error.stack || '(no stack trace available)'),
  ];

  if (errorInfo?.componentStack) {
    lines.push('', 'Component Stack:', sanitizeTelemetry(errorInfo.componentStack));
  }

  return lines.join('\n');
}

function generateCrashBundle(error, errorInfo) {
  const bundle = {
    appVersion: '1.0.0',
    timestamp: new Date().toISOString(),
    userAgent: 'test-agent',
    url: 'http://localhost/test',
    error: {
      name: error.name || 'Error',
      message: sanitizeTelemetry(error.message || ''),
      stack: sanitizeTelemetry(error.stack || ''),
    },
    componentStack: errorInfo?.componentStack
      ? sanitizeTelemetry(errorInfo.componentStack)
      : null,
  };
  return JSON.stringify(bundle, null, 2);
}

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

// ── Telemetry Sanitizer & Crash Bundle Tests (#1453) ──────────────────────────

test('sanitizeTelemetry: redacts Stellar secret keys, bearer tokens, and api keys', () => {
  const rawText = 'Error connecting with Bearer eyJhbGciOi.token and api_key=secret123 and SAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA';
  const sanitized = sanitizeTelemetry(rawText);

  assert.ok(!sanitized.includes('secret123'));
  assert.ok(!sanitized.includes('SAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'));
  assert.ok(!sanitized.includes('eyJhbGciOi.token'));
  assert.ok(sanitized.includes('Bearer [REDACTED_BEARER_TOKEN]'));
  assert.ok(sanitized.includes('[REDACTED_STELLAR_SECRET_KEY]'));
  assert.ok(sanitized.includes('api_key=[REDACTED]'));
});

test('buildDiagnosticsReport: applies sanitization to message and stack trace', () => {
  const err = new Error('Failed with token=supersecret123');
  err.stack = 'Error: Failed with token=supersecret123\n at func (http://localhost/app.js)';
  const report = buildDiagnosticsReport(err, { componentStack: 'at Component (token=secret)' });

  assert.ok(!report.includes('supersecret123'));
  assert.ok(report.includes('token=[REDACTED]'));
});

test('generateCrashBundle: returns valid sanitized JSON bundle', () => {
  const err = new Error('RPC crash api_key=xyz');
  const bundleJson = generateCrashBundle(err, null);
  const parsed = JSON.parse(bundleJson);

  assert.equal(parsed.appVersion, '1.0.0');
  assert.ok(parsed.timestamp);
  assert.equal(parsed.error.name, 'Error');
  assert.ok(!parsed.error.message.includes('xyz'));
  assert.ok(parsed.error.message.includes('api_key=[REDACTED]'));
});
