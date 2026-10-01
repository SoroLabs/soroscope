import React, { useState, useEffect, useCallback } from "react";
import Link from "next/link";
import { useRouter } from "next/router";
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
  Calculator,
  BarChart3,
  Binary,
} from "lucide-react";
import { useTheme } from "next-themes";
import { ConnectButton } from "./ConnectButton";
import { NetworkSwitcher } from "./NetworkSwitcher";

export type NavTab = "explorer" | "history" | "transactions" | "schema" | "disassembly";

const NAV_TABS: { id: NavTab; label: string; Icon: typeof Layers }[] = [
  { id: "explorer", label: "Result", Icon: Layers },
  { id: "schema", label: "Schema", Icon: Network },
  { id: "disassembly", label: "Disassembly", Icon: Binary },
  { id: "history", label: "History", Icon: History },
  { id: "transactions", label: "Transactions", Icon: List },
];

type QuickNavItem =
  | { id: string; label: string; Icon: typeof Activity; kind: "tab"; tab: NavTab; href: string }
  | { id: string; label: string; Icon: typeof Activity; kind: "route"; href: string };

export const HEADER_QUICK_LINKS: QuickNavItem[] = [
  { id: "simulator", label: "Simulator", Icon: Activity, kind: "tab", tab: "explorer", href: "/?tab=explorer#simulator" },
  { id: "analytics", label: "Analytics", Icon: BarChart3, kind: "tab", tab: "schema", href: "/?tab=schema#analytics" },
  { id: "staking", label: "Staking Calculator", Icon: Calculator, kind: "tab", tab: "explorer", href: "/?tab=explorer#staking-calculator" },
  { id: "settings", label: "Settings", Icon: Settings, kind: "route", href: "/settings" },
];

export type CongestionLevel = "low" | "medium" | "high";

export interface CongestionState {
  level: CongestionLevel;
  baseFee: number;
  recommendedFee: number;
}

const CONGESTION_STYLES: Record<
  CongestionLevel,
  { label: string; dot: string; badge: string; text: string }
> = {
  low: {
    label: "Low",
    dot: "bg-emerald-400",
    badge: "border-emerald-500/40 bg-emerald-500/10",
    text: "text-emerald-300",
  },
  medium: {
    label: "Medium",
    dot: "bg-amber-400",
    badge: "border-amber-500/40 bg-amber-500/10",
    text: "text-amber-300",
  },
  high: {
    label: "High",
    dot: "bg-red-500",
    badge: "border-red-500/40 bg-red-500/10",
    text: "text-red-300",
  },
};

export function classifyCongestion(baseFee: number): CongestionLevel {
  if (baseFee >= 1000) return "high";
  if (baseFee >= 200) return "medium";
  return "low";
}

const FALLBACK_BASE_FEE = 100;
const CONGESTION_POLL_MS = 15000;

async function fetchLedgerBaseFee(): Promise<number> {
  const res = await fetch("/api/network/fees", {
    headers: { accept: "application/json" },
  });
  if (!res.ok) throw new Error(`Fee endpoint responded ${res.status}`);
  const data = (await res.json()) as { baseFee?: number; base_fee?: number };
  const fee = data.baseFee ?? data.base_fee;
  if (typeof fee !== "number" || !Number.isFinite(fee)) {
    throw new Error("Malformed fee payload");
  }
  return fee;
}

export function isHeaderQuickLinkActive(item: QuickNavItem, activeTab: NavTab, pathname: string): boolean {
  if (item.kind === "route") return pathname === item.href;
  return activeTab === item.tab;
}

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
  const router = useRouter();
  const { theme, setTheme } = useTheme();
  const [mounted, setMounted] = useState(false);

  const [congestion, setCongestion] = useState<CongestionState>({
    level: "low",
    baseFee: FALLBACK_BASE_FEE,
    recommendedFee: FALLBACK_BASE_FEE,
  });

  const refreshCongestion = useCallback(async () => {
    try {
      const baseFee = await fetchLedgerBaseFee();
      setCongestion({
        level: classifyCongestion(baseFee),
        baseFee,
        recommendedFee: Math.max(baseFee, Math.ceil(baseFee * 1.5)),
      });
    } catch {
      setCongestion((prev) => ({
        ...prev,
        recommendedFee: Math.max(prev.baseFee, Math.ceil(prev.baseFee * 1.5)),
      }));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    const tick = () => {
      if (!cancelled) void refreshCongestion();
    };
    tick();
    const interval = window.setInterval(tick, CONGESTION_POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [refreshCongestion]);

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

  const handleQuickTabSelect = (selectedTab: NavTab) => {
    handleSelectTab(selectedTab);
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
          <CongestionBadge congestion={congestion} />
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
          <CongestionBadge congestion={congestion} compact />
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
                <p className="px-1 text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">
                  Quick navigation
                </p>
                {HEADER_QUICK_LINKS.map((item) => {
                  const Icon = item.Icon;
                  const isActive = isHeaderQuickLinkActive(item, tab, router.pathname);
                  const className = `flex min-h-[48px] w-full items-center gap-3 rounded-xl px-4 py-3.5 text-base font-medium transition-colors ${
                    isActive
                      ? "bg-cyan-500/10 text-cyan-400 border border-cyan-500/30 font-semibold"
                      : "text-slate-300 hover:bg-slate-800/80 hover:text-white"
                  }`;

                  if (item.kind === "route") {
                    return (
                      <Link
                        key={item.id}
                        href={item.href}
                        onClick={() => setMobileMenuOpen(false)}
                        aria-current={isActive ? "page" : undefined}
                        className={className}
                      >
                        <Icon className="h-5 w-5" />
                        <span>{item.label}</span>
                      </Link>
                    );
                  }

                  return (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => handleQuickTabSelect(item.tab)}
                      aria-current={isActive ? "page" : undefined}
                      className={className}
                    >
                      <Icon className="h-5 w-5" />
                      <span>{item.label}</span>
                    </button>
                  );
                })}

                <p className="px-1 pt-4 text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500">
                  Analyzer panels
                </p>
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

interface CongestionBadgeProps {
  congestion: CongestionState;
  compact?: boolean;
}

export function CongestionBadge({ congestion, compact = false }: CongestionBadgeProps) {
  const style = CONGESTION_STYLES[congestion.level];
  const tooltip = `Soroban network congestion: ${style.label}. Base fee: ${congestion.baseFee} stroops. Recommended fee: ${congestion.recommendedFee} stroops.`;

  return (
    <div
      role="status"
      aria-live="polite"
      aria-label={tooltip}
      title={tooltip}
      data-congestion-level={congestion.level}
      className={`group relative flex min-h-[44px] items-center gap-2 rounded-lg border px-3 py-2 text-xs font-medium ${style.badge} ${style.text}`}
    >
      <span className="relative flex h-2 w-2">
        <span
          className={`absolute inline-flex h-full w-full animate-ping rounded-full opacity-75 ${style.dot}`}
        />
        <span className={`relative inline-flex h-2 w-2 rounded-full ${style.dot}`} />
      </span>
      <Wifi className="h-4 w-4" aria-hidden="true" />
      {!compact && (
        <span className="hidden lg:inline">
          {style.label} congestion
        </span>
      )}
      <span className="pointer-events-none absolute left-1/2 top-full z-50 mt-2 hidden -translate-x-1/2 whitespace-nowrap rounded-md border border-slate-700 bg-slate-900 px-3 py-2 text-[11px] font-normal text-slate-200 shadow-lg group-hover:block group-focus-within:block">
        {tooltip}
      </span>
    </div>
  );
}
