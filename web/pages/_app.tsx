import type { AppProps } from "next/app";
import "../styles/globals.css";
import { ThemeProvider } from "next-themes";
import { NetworkProvider } from "../context/NetworkContext";
import { WalletProvider } from "../context/WalletContext";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { GlobalSearchModal } from "../components/GlobalSearchModal";

export default function App({ Component, pageProps }: AppProps) {
  return (
    <ErrorBoundary>
      <ThemeProvider attribute="class" defaultTheme="dark" enableSystem>
        <NetworkProvider>
          <WalletProvider>
            <OfflineBanner />
            <Component {...pageProps} />
            {/* Mounted app-wide so Cmd+K / Ctrl+K works on every page. */}
            <GlobalSearchModal />
          </WalletProvider>
        </NetworkProvider>
      </ThemeProvider>
    </ErrorBoundary>
  );
}
