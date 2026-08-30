import React, { useState, useEffect } from "react";
import Link from "next/link";
import {
  Menu,
  X,
  Layers,
  History,
  Activity,
  List,
  Sun,
  Moon,
  Network,
  Search,
  Settings,
} from "lucide-react";
import { Menu, X, Layers, History, Activity, List, Sun, Moon, TrendingUp } from "lucide-react";
import { useTheme } from "next-themes";
import { ConnectButton } from "./ConnectButton";
import { NetworkSwitcher } from "./NetworkSwitcher";

export type NavTab = "explorer" | "history" | "transactions" | "schema";

const NAV_TABS: { id: NavTab; label: string; Icon: typeof Layers }[] = [
  { id: "explorer", label: "Result", Icon: Layers },
  { id: "schema", label: "Schema", Icon: Network },
  { id: "history", label: "History", Icon: History },
  { id: "transactions", label: "Transactions", Icon: List },
];
export type NavTab = "explorer" | "history" | "transactions" | "analytics";

interface HeaderNavProps {
  tab: NavTab;
  setTab: (tab: NavTab) => void;
}

/** Ask the app-wide overlay to open, same as pressing Cmd+K. */
function openGlobalSearch() {
  window.dispatchEvent(
    new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true }),
  );
}

export function HeaderNav({ tab, setTab }: HeaderNavProps) {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const { theme, setTheme } = useTheme();
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  // Close drawer on Escape key press
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && mobileMenuOpen) {
        setMobileMenuOpen(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [mobileMenuOpen]);

  // Lock body scroll when mobile drawer is open
  useEffect(() => {
    if (mobileMenuOpen) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "";
    }
    return () => {
      document.body.style.overflow = "";
    };
  }, [mobileMenuOpen]);

  const handleSelectTab = (selectedTab: NavTab) => {
    setTab(selectedTab);
    setMobileMenuOpen(false);
  };

  return (
    <header className="sticky top-0 z-50 border-b border-slate-800 bg-slate-950/90 backdrop-blur">
      {/* Top Header Bar */}
      <div className="mx-auto flex max-w-6xl items-center justify-between px-4 py-4 sm:px-6 lg:px-8">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-tr from-cyan-500 to-blue-600 shadow-md shadow-cyan-500/20">
            <Activity className="h-5 w-5 text-slate-950 font-bold" />
          </div>
          <div>
            <h1 className="text-xl font-bold tracking-tight text-white sm:text-2xl">
              Soro<span className="text-cyan-400">Scope</span>
            </h1>
            <p className="text-xs text-slate-400 hidden sm:block">
              Soroban smart contract resource analyzer
            </p>
          </div>
        </div>

        {/* Desktop Navigation & Actions */}
        <div className="hidden sm:flex sm:items-center sm:gap-4">
          <button
            type="button"
            onClick={openGlobalSearch}
            aria-label="Open global search (Control K)"
            className="flex min-h-[44px] items-center gap-2 rounded-lg border border-slate-800 bg-slate-900 px-3 py-2 text-sm text-slate-400 transition-colors hover:bg-slate-800 hover:text-white focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
          >
            <Search className="h-4 w-4" />
            <span className="hidden lg:inline">Search</span>
            <kbd className="rounded border border-slate-700 bg-slate-950 px-1.5 py-0.5 font-mono text-[10px] text-slate-500">
              ⌘K
            </kbd>
          </button>
          <Link
            href="/settings"
            aria-label="Open settings"
            className="flex min-h-[44px] min-w-[44px] items-center justify-center rounded-lg border border-slate-800 bg-slate-900 p-2.5 text-slate-300 transition-colors hover:bg-slate-800 hover:text-white focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
          >
            <Settings className="h-5 w-5" />
          </Link>
          <NetworkSwitcher />
          {mounted && (
            <button
              type="button"
              onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
              aria-label={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
              className="flex min-h-[44px] min-w-[44px] items-center justify-center rounded-lg border border-slate-800 bg-slate-900 p-2.5 text-slate-300 transition-colors hover:bg-slate-800 hover:text-white focus:outline-none focus:ring-2 focus:ring-cyan-500/50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400 dark:hover:bg-slate-700 dark:hover:text-white"
            >
              {theme === 'dark' ? <Sun className="h-5 w-5" /> : <Moon className="h-5 w-5" />}
            </button>
          )}
          <ConnectButton />
        </div>

        {/* Mobile Hamburger Button (< 640px) */}
        <div className="flex items-center gap-2 sm:hidden">
          <NetworkSwitcher />
          <ConnectButton />
          <button
            type="button"
            onClick={() => setMobileMenuOpen(true)}
            aria-label="Open mobile navigation menu"
            aria-expanded={mobileMenuOpen}
            aria-controls="mobile-navigation-drawer"
            className="flex min-h-[44px] min-w-[44px] items-center justify-center rounded-lg border border-slate-800 bg-slate-900 p-2.5 text-slate-300 transition-colors hover:bg-slate-800 hover:text-white focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
          >
            <Menu className="h-6 w-6" />
          </button>
        </div>
      </div>

      {/* Desktop Tabs Bar (>= 640px) */}
      <div className="hidden sm:flex border-t border-slate-800/80 bg-slate-950/60 px-4 sm:px-6 lg:px-8">
        <div className="mx-auto flex w-full max-w-6xl">
          {NAV_TABS.map(({ id, label, Icon }) => (
            <button
              key={id}
              type="button"
              onClick={() => setTab(id)}
              aria-current={tab === id ? "page" : undefined}
              className={`flex items-center gap-2 border-b-2 px-6 py-3 text-sm font-medium transition-colors ${
                tab === id
                  ? "border-cyan-400 text-cyan-400 bg-cyan-950/20"
                  : "border-transparent text-slate-400 hover:border-slate-700 hover:text-slate-200"
              }`}
            >
              <Icon className="h-4 w-4" />
              {label}
            </button>
          ))}
            onClick={() => setTab("explorer")}
              tab === "explorer"
            <Layers className="h-4 w-4" />
            Result
            onClick={() => setTab("history")}
              tab === "history"
            <History className="h-4 w-4" />
            History
            onClick={() => setTab("transactions")}
              tab === "transactions"
            <List className="h-4 w-4" />
            Transactions
            onClick={() => setTab("analytics")}
              tab === "analytics"
            <TrendingUp className="h-4 w-4" />
            LP Analytics
        </div>
      </div>

      {/* Mobile Navigation Drawer (Slide-Over Menu) */}
      {mobileMenuOpen && (
        <div
          className="fixed inset-0 z-50 sm:hidden"
          role="dialog"
          aria-modal="true"
          aria-label="Mobile Navigation Menu"
          id="mobile-navigation-drawer"
        >
          {/* Overlay Backdrop */}
          <div
            className="fixed inset-0 bg-slate-950/80 backdrop-blur-sm transition-opacity"
            onClick={() => setMobileMenuOpen(false)}
            aria-hidden="true"
          />

          {/* Drawer Slide-Over Content */}
          <div className="fixed inset-y-0 right-0 z-50 flex w-full max-w-xs flex-col justify-between border-l border-slate-800 bg-slate-900 p-6 shadow-2xl">
            {/* Drawer Header */}
            <div>
              <div className="flex items-center justify-between border-b border-slate-800 pb-4">
                <div className="flex items-center gap-2">
                  <Activity className="h-5 w-5 text-cyan-400" />
                  <span className="font-bold text-white">SoroScope Menu</span>
                </div>
                <button
                  type="button"
                  onClick={() => setMobileMenuOpen(false)}
                  aria-label="Close mobile navigation menu"
                  className="flex min-h-[44px] min-w-[44px] items-center justify-center rounded-lg border border-slate-800 bg-slate-950/60 p-2.5 text-slate-400 transition-colors hover:bg-slate-800 hover:text-white focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
                >
                  <X className="h-6 w-6" />
                </button>
              </div>

              {/* Drawer Links */}
              <nav className="mt-6 flex flex-col gap-2">
                {NAV_TABS.map(({ id, label, Icon }) => (
                  <button
                    key={id}
                    type="button"
                    onClick={() => handleSelectTab(id)}
                    aria-current={tab === id ? "page" : undefined}
                    className={`flex min-h-[48px] w-full items-center gap-3 rounded-xl px-4 py-3.5 text-base font-medium transition-colors ${
                      tab === id
                        ? "bg-cyan-500/10 text-cyan-400 border border-cyan-500/30 font-semibold"
                        : "text-slate-300 hover:bg-slate-800/80 hover:text-white"
                    }`}
                  >
                    <Icon className="h-5 w-5" />
                    <span>{label}</span>
                  </button>
                ))}

                <button
                  type="button"
                  onClick={() => {
                    setMobileMenuOpen(false);
                    openGlobalSearch();
                  }}
                  className="flex min-h-[48px] w-full items-center gap-3 rounded-xl px-4 py-3.5 text-base font-medium text-slate-300 transition-colors hover:bg-slate-800/80 hover:text-white"
                >
                  <Search className="h-5 w-5" />
                  <span>Search</span>
                </button>

                <Link
                  href="/settings"
                  onClick={() => setMobileMenuOpen(false)}
                  className="flex min-h-[48px] w-full items-center gap-3 rounded-xl px-4 py-3.5 text-base font-medium text-slate-300 transition-colors hover:bg-slate-800/80 hover:text-white"
                >
                  <Settings className="h-5 w-5" />
                  <span>Settings</span>
                </Link>
              </nav>
            </div>

            {/* Drawer Footer info */}
            <div className="border-t border-slate-800 pt-4">
              <div className="mb-3">
                <NetworkSwitcher isMobile={true} />
              </div>
              {mounted && (
                <div className="mb-4 flex items-center justify-center gap-2">
                  <button
                    type="button"
                    onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
                    aria-label={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
                    className="flex w-full min-h-[48px] items-center justify-center gap-3 rounded-xl border border-slate-700 bg-slate-800 px-4 py-3.5 text-base font-medium text-slate-400 transition-colors hover:bg-slate-700 hover:text-white"
                  >
                    {theme === 'dark' ? (
                      <><Sun className="h-5 w-5" /><span>Light Mode</span></>
                    ) : (
                      <><Moon className="h-5 w-5" /><span>Dark Mode</span></>
                    )}
                  </button>
                </div>
              )}
              <p className="text-center text-xs text-slate-500">
                Soroban Resource Analyzer &bull; SoroScope
              </p>
            </div>
          </div>
        </div>
      )}
    </header>
  );
}
