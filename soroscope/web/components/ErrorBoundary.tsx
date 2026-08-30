import React from 'react';

type ErrorBoundaryProps = {
  children: React.ReactNode;
  fallback?: (error: Error, reset: () => void) => React.ReactNode;
};

type ErrorBoundaryState = {
  error: Error | null;
};

export class ErrorBoundary extends React.Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Unhandled UI error:', error, errorInfo);
  }

  reset = () => {
    this.setState({ error: null });
  };

  render() {
    if (this.state.error) {
      if (this.props.fallback) {
        return this.props.fallback(this.state.error, this.reset);
      }

      return (
        <div className="rounded-lg border border-red-800/60 bg-red-950/30 p-4 text-red-100">
          <p className="text-sm font-semibold">Something went wrong</p>
          <p className="mt-1 text-xs text-red-200/80">{this.state.error.message}</p>
          <button
            type="button"
            onClick={this.reset}
            className="mt-3 rounded-md border border-red-700/70 px-3 py-1.5 text-xs text-red-100 hover:bg-red-900/40"
          >
            Try again
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
