"use client";

import { useMemo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { RefreshCw, TrendingUp, TrendingDown, Wallet } from "lucide-react";
import { useWallet } from "../context/WalletContext";
import { useTokenPrices } from "../hooks/useTokenPrices";
import { AssetBalance } from "../lib/stellarBalances";
import { PriceInfo } from "../lib/priceFeed";
import { cn } from "../lib/utils";

const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

const amountFmt = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 7,
});

function formatBalance(raw: string): string {
  const n = Number(raw);
  return Number.isFinite(n) ? amountFmt.format(n) : raw;
}

function assetValueUsd(balance: AssetBalance, price?: PriceInfo): number | null {
  if (!price) return null;
  const n = Number(balance.balance);
  return Number.isFinite(n) ? n * price.usd : null;
}

function AssetRow({
  asset,
  price,
}: {
  asset: AssetBalance;
  price?: PriceInfo;
}) {
  const value = assetValueUsd(asset, price);
  const change = price?.change24h ?? null;
  const up = change !== null && change >= 0;

  return (
    <div className="flex items-center justify-between gap-3 rounded-xl border border-[#1e293b] bg-[#0F1621] px-4 py-3">
      <div className="flex items-center gap-3 min-w-0">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-[#33C5E0]/10 text-sm font-bold text-[#33C5E0]">
          {asset.symbol.slice(0, 3).toUpperCase()}
        </div>
        <div className="min-w-0">
          <p className="truncate text-sm font-semibold text-slate-100">
            {asset.symbol}
            {asset.isNative && (
              <span className="ml-1.5 text-[10px] font-medium uppercase tracking-wide text-slate-500">
                native
              </span>
            )}
          </p>
          <p className="truncate font-mono text-xs text-slate-400">
            {formatBalance(asset.balance)}
          </p>
        </div>
      </div>

      <div className="text-right">
        {value !== null ? (
          <p className="text-sm font-semibold text-slate-100">
            {usd.format(value)}
          </p>
        ) : (
          <p className="text-xs text-slate-500">no price</p>
        )}
        {change !== null && (
          <p
            className={cn(
              "flex items-center justify-end gap-0.5 text-xs font-medium",
              up ? "text-emerald-400" : "text-red-400",
            )}
          >
            {up ? (
              <TrendingUp className="h-3 w-3" />
            ) : (
              <TrendingDown className="h-3 w-3" />
            )}
            {Math.abs(change).toFixed(2)}%
          </p>
        )}
      </div>
    </div>
  );
}

/**
 * Multi-asset wallet balance card with real-time USD price feeds.
 *
 * Reads balances from `WalletContext` (which clears them on disconnect) and
 * layers live prices from `useTokenPrices` on top, showing a portfolio total
 * plus a per-asset breakdown.
 */
export function WalletBalanceCard() {
  const {
    isConnected,
    balances,
    balancesLoading,
    balancesError,
    refreshBalances,
  } = useWallet();

  const symbols = useMemo(
    () => balances.map((b) => b.symbol),
    [balances],
  );
  const {
    prices,
    loading: pricesLoading,
    error: pricesError,
    lastUpdated,
  } = useTokenPrices(symbols);

  const totalUsd = useMemo(() => {
    return balances.reduce((sum, b) => {
      const v = assetValueUsd(b, prices[b.symbol]);
      return v !== null ? sum + v : sum;
    }, 0);
  }, [balances, prices]);

  if (!isConnected) return null;

  const refreshing = balancesLoading || pricesLoading;

  return (
    <div className="w-full rounded-2xl border border-[#1e293b] bg-[#0B111A] p-5">
      <div className="mb-4 flex items-start justify-between gap-3">
        <div className="flex items-center gap-2">
          <Wallet className="h-5 w-5 text-[#33C5E0]" />
          <div>
            <h3 className="text-sm font-semibold text-slate-100">
              Wallet Balances
            </h3>
            <p className="text-xs text-slate-500">
              {lastUpdated
                ? `Prices updated ${new Date(lastUpdated).toLocaleTimeString()}`
                : "Live price feed"}
            </p>
          </div>
        </div>
        <button
          onClick={refreshBalances}
          className="rounded-lg border border-[#1e293b] p-2 text-slate-400 transition-colors hover:border-[#33C5E0]/50 hover:text-[#33C5E0]"
          aria-label="Refresh balances"
        >
          <RefreshCw
            className={cn("h-4 w-4", refreshing && "animate-spin")}
          />
        </button>
      </div>

      <div className="mb-4 rounded-xl bg-gradient-to-br from-[#33C5E0]/10 to-transparent px-4 py-3">
        <p className="text-xs uppercase tracking-wide text-slate-400">
          Total Value
        </p>
        <p className="text-2xl font-bold text-slate-50">
          {usd.format(totalUsd)}
        </p>
      </div>

      {balancesError && (
        <p className="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-400">
          {balancesError}
        </p>
      )}
      {!balancesError && pricesError && (
        <p className="mb-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-400">
          Prices unavailable: {pricesError}
        </p>
      )}

      <div className="space-y-2">
        <AnimatePresence initial={false}>
          {balances.length === 0 && !balancesLoading && !balancesError && (
            <p className="py-6 text-center text-sm text-slate-500">
              No assets held on this account.
            </p>
          )}
          {balancesLoading && balances.length === 0 && (
            <div className="space-y-2">
              {[0, 1, 2].map((i) => (
                <div
                  key={i}
                  className="h-16 animate-pulse rounded-xl border border-[#1e293b] bg-[#0F1621]"
                />
              ))}
            </div>
          )}
          {balances.map((asset) => (
            <motion.div
              key={asset.id}
              layout
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={{ duration: 0.15 }}
            >
              <AssetRow asset={asset} price={prices[asset.symbol]} />
            </motion.div>
          ))}
        </AnimatePresence>
      </div>
    </div>
  );
}

export default WalletBalanceCard;
