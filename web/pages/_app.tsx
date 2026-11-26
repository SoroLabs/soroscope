import type { AppProps } from "next/app";
import { Inter, JetBrains_Mono } from "next/font/google";
import "../styles/globals.css";
import { ThemeProvider } from "next-themes";
import { NetworkProvider } from "../context/NetworkContext";
import { TelemetryProvider } from "../context/TelemetryContext";
import { WalletProvider } from "../context/WalletContext";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { GlobalSearchModal } from "../components/GlobalSearchModal";
import { KeyboardShortcutsModal } from "../components/KeyboardShortcutsModal";
import { OfflineBanner } from "../components/OfflineBanner";
import { ToastHost } from "../components/Toast";
import { useEffect } from "react";

const inter = Inter({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-inter",
});

const jetBrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-jetbrains-mono",
});

export default function App({ Component, pageProps }: AppProps) {
  // Register the service worker for offline support (issue #1300).
  useEffect(() => {
    if (
      typeof window !== "undefined" &&
      "serviceWorker" in navigator &&
      process.env.NODE_ENV === "production"
    ) {
      navigator.serviceWorker
        .register("/sw.js")
        .catch((error) => {
          console.warn("Service worker registration failed:", error);
        });
    }
  }, []);

  return (
    <div className={`${inter.variable} ${jetBrainsMono.variable} font-sans`}>
      <ErrorBoundary>
        <ThemeProvider attribute="class" defaultTheme="dark" enableSystem>
          <TelemetryProvider>
            <NetworkProvider>
              <WalletProvider>
                <OfflineBanner />
                <Component {...pageProps} />
                {/* Mounted app-wide so Cmd+K / Ctrl+K works on every page. */}
                <GlobalSearchModal />
                {/* Mounted app-wide so ? opens the shortcuts cheat sheet on every page. */}
                <KeyboardShortcutsModal />
                <ToastHost />
              </WalletProvider>
            </NetworkProvider>
          </TelemetryProvider>
        </ThemeProvider>
      </ErrorBoundary>
    </div>
  );
}
