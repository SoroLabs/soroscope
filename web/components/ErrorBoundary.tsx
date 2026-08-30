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
                    onClick={() => window.location.reload()}
                    className="rounded-lg border border-slate-700 bg-slate-800/80 px-4 py-2.5 text-sm font-semibold text-slate-100 transition-colors hover:bg-slate-700 focus:outline-none focus:ring-2 focus:ring-slate-500/50"
                  >
                    Reload dashboard
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
