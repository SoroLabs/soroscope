import React from 'react';

export type ErrorBoundaryProps = {
  children: React.ReactNode;
  fallback?: (error: Error, reset: () => void) => React.ReactNode;
  title?: string;
  description?: string;
};

export type ErrorBoundaryState = {
  error: Error | null;
  errorInfo: React.ErrorInfo | null;
  showDetails: boolean;
};

/**
 * Helper to check if an error is an RPC or network fetch failure.
 */
export function isRpcNetworkError(error: Error | null): boolean {
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

/**
 * Sanitizes sensitive tokens, secret keys, and credentials from error messages and stack traces.
 */
export function sanitizeTelemetry(text: string): string {
  if (!text) return '';
  return text
    // Stellar secret keys (starts with S, 56 characters base32)
    .replace(/\bS[A-Z2-7]{55}\b/g, '[REDACTED_STELLAR_SECRET_KEY]')
    // Bearer tokens
    .replace(/Bearer\s+[A-Za-z0-9\-_.~+/]+=*/gi, 'Bearer [REDACTED_BEARER_TOKEN]')
    // Authorization headers or key=val tokens
    .replace(/(api[_-]?key|secret|token|password|auth|authorization)=["']?[^&"'\s]+/gi, '$1=[REDACTED]')
    // Hex private keys (32 or 64 bytes)
    .replace(/\b(0x)?[0-9a-fA-F]{64}\b/g, '[REDACTED_HEX_KEY]');
}

/**
 * Builds a plain-text diagnostics report for the current error state with sanitization.
 */
export function buildDiagnosticsReport(
  error: Error,
  errorInfo: React.ErrorInfo | null,
): string {
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

/**
 * Generates a sanitized JSON crash bundle for telemetry export.
 */
export function generateCrashBundle(
  error: Error,
  errorInfo: React.ErrorInfo | null,
): string {
  const bundle = {
    appVersion: '1.0.0',
    timestamp: new Date().toISOString(),
    userAgent: typeof navigator !== 'undefined' ? navigator.userAgent : 'unknown',
    url: typeof window !== 'undefined' ? window.location.href : 'unknown',
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

/**
 * Resets cached application state in LocalStorage while preserving sensitive user secrets if any.
 */
export function resetCachedAppState(): void {
  if (typeof window === 'undefined' || !window.localStorage) return;
  try {
    const keysToPreserve = ['encrypted_key', 'wallet_auth', 'user_credentials'];
    const preserved: Record<string, string> = {};
    for (const key of keysToPreserve) {
      const val = window.localStorage.getItem(key);
      if (val !== null) preserved[key] = val;
    }
    window.localStorage.clear();
    for (const [k, v] of Object.entries(preserved)) {
      window.localStorage.setItem(k, v);
    }
  } catch (err) {
    console.error('Failed to reset cached state:', err);
  }
}

export class ErrorBoundary extends React.Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = {
    error: null,
    errorInfo: null,
    showDetails: false,
  };

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return { error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    this.setState({ errorInfo });
    console.error('Unhandled UI error:', error, errorInfo);
  }

  reset = () => {
    this.setState({ error: null, errorInfo: null, showDetails: false });
  };

  toggleDetails = () => {
    this.setState((prevState) => ({ showDetails: !prevState.showDetails }));
  };

  copyDiagnostics = async () => {
    const { error, errorInfo } = this.state;
    if (!error) return;

    const report = buildDiagnosticsReport(error, errorInfo);

    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(report);
      } else if (typeof window !== 'undefined') {
        window.prompt('Copy diagnostics:', report);
      }
    } catch (copyError) {
      console.error('Failed to copy diagnostics:', copyError);
    }
  };

  resetAndReload = () => {
    resetCachedAppState();
    if (typeof window !== 'undefined') {
      window.location.reload();
    }
  };

  exportCrashBundle = () => {
    const { error, errorInfo } = this.state;
    if (!error) return;
    const json = generateCrashBundle(error, errorInfo);
    try {
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `crash-report-${Date.now()}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error('Failed to export crash bundle:', err);
    }
  };

  render() {
    const { error, errorInfo, showDetails } = this.state;

    if (error) {
      if (this.props.fallback) {
        return this.props.fallback(error, this.reset);
      }

      const isRpcError = isRpcNetworkError(error);
      const defaultTitle = isRpcError
        ? 'RPC Failure Recovery'
        : 'Something went wrong';
      const defaultDescription = isRpcError
        ? 'Uncaught network error during RPC fetch. The RPC node may be unreachable or experiencing network latency.'
        : 'A dashboard component crashed while rendering. You can retry without losing the rest of the app.';

      return (
        <div
          role="alert"
          aria-live="assertive"
          className="flex min-h-[320px] items-center justify-center rounded-xl border border-red-900/60 bg-[var(--bg-elevated)] p-6 text-red-100 shadow-2xl"
        >
          <div className="w-full max-w-xl rounded-xl border border-red-800/60 bg-red-950/30 p-6 shadow-xl shadow-black/20">
            <div className="flex items-start gap-4">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full border border-red-700/70 bg-red-950 text-lg font-bold text-red-400">
                !
              </div>
              <div className="min-w-0 flex-1">
                <h2 className="text-lg font-bold text-red-50">
                  {this.props.title ?? defaultTitle}
                </h2>
                <p className="mt-2 text-sm leading-6 text-red-100/80">
                  {this.props.description ?? defaultDescription}
                </p>
                <p className="mt-3 rounded-md border border-red-900/70 bg-black/40 px-3 py-2 font-mono text-xs text-red-200">
                  {error.message || error.name}
                </p>

                {/* Error Stack Toggle Button */}
                <div className="mt-4">
                  <button
                    type="button"
                    onClick={this.toggleDetails}
                    aria-expanded={showDetails}
                    className="inline-flex items-center gap-1.5 text-xs font-semibold text-red-300 hover:text-red-100 hover:underline focus:outline-none"
                  >
                    <span>{showDetails ? 'Hide technical details' : 'Show technical details'}</span>
                    <span>{showDetails ? '▲' : '▼'}</span>
                  </button>

                  {showDetails && (
                    <div className="mt-3 max-h-48 overflow-auto rounded-md border border-red-900/80 bg-black/60 p-3 font-mono text-xs text-red-200/90 whitespace-pre-wrap">
                      <p className="font-semibold text-red-400">Stack Trace:</p>
                      <p className="mt-1">{error.stack || 'No error stack available.'}</p>
                      {errorInfo?.componentStack && (
                        <>
                          <p className="mt-2 font-semibold text-red-400">Component Stack:</p>
                          <p className="mt-1">{errorInfo.componentStack}</p>
                        </>
                      )}
                    </div>
                  )}
                </div>

                {/* User-Friendly Retry Controls */}
                <div className="mt-6 flex flex-wrap items-center gap-3">
                  <button
                    type="button"
                    onClick={this.reset}
                    className="rounded-lg border border-red-700/80 bg-red-900/50 px-4 py-2.5 text-sm font-semibold text-red-50 transition-colors hover:bg-red-800/70 focus:outline-none focus:ring-2 focus:ring-red-500/50"
                  >
                    Try again
                  </button>
                  <button
                    type="button"
                    onClick={this.resetAndReload}
                    className="rounded-lg border border-purple-700/80 bg-purple-900/40 px-4 py-2.5 text-sm font-semibold text-purple-50 transition-colors hover:bg-purple-800/60 focus:outline-none focus:ring-2 focus:ring-purple-500/50"
                  >
                    Reset Cached State &amp; Reload
                  </button>
                  <button
                    type="button"
                    onClick={this.copyDiagnostics}
                    className="rounded-lg border border-amber-700/80 bg-amber-900/40 px-4 py-2.5 text-sm font-semibold text-amber-50 transition-colors hover:bg-amber-800/60 focus:outline-none focus:ring-2 focus:ring-amber-500/50"
                  >
                    Copy Diagnostics
                  </button>
                  <button
                    type="button"
                    onClick={this.exportCrashBundle}
                    className="rounded-lg border border-blue-700/80 bg-blue-900/40 px-4 py-2.5 text-sm font-semibold text-blue-50 transition-colors hover:bg-blue-800/60 focus:outline-none focus:ring-2 focus:ring-blue-500/50"
                  >
                    Export Crash Bundle
                  </button>
                  <button
                    type="button"
                    onClick={() => window.location.reload()}
                    className="rounded-lg border border-slate-700 bg-slate-800/80 px-4 py-2.5 text-sm font-semibold text-slate-100 transition-colors hover:bg-slate-700 focus:outline-none focus:ring-2 focus:ring-slate-500/50"
                  >
                    Reload Application
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
