import type { AppProps } from 'next/app';
import '../styles/globals.css';
import { WalletProvider } from '../context/WalletContext';
import { ErrorBoundary } from '../components/ErrorBoundary';
import { ToastProvider } from '../components/ToastViewport';

export default function App({ Component, pageProps }: AppProps) {
  return (
    <ErrorBoundary>
      <WalletProvider>
        <ToastProvider>
          <Component {...pageProps} />
        </ToastProvider>
      </WalletProvider>
    </ErrorBoundary>
  );
}
