import type { AppProps } from "next/app";
import { Inter, JetBrains_Mono } from "next/font/google";
import "../styles/globals.css";
import { ThemeProvider } from "next-themes";
import { NetworkProvider } from "../context/NetworkContext";
import { WalletProvider } from "../context/WalletContext";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { GlobalSearchModal } from "../components/GlobalSearchModal";

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
  return (
    <div className={`${inter.variable} ${jetBrainsMono.variable} font-sans`}>
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
    </div>
  );
}
