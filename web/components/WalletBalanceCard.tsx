"use client";

import { useMemo, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  RefreshCw,
  TrendingUp,
  TrendingDown,
  Wallet,
  AlertCircle,
} from "lucide-react";
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
  const value = Number(raw);

  return Number.isFinite(value) ? amountFmt.format(value) : raw;
}

function assetValueUsd(
  balance: AssetBalance,
  price?: PriceInfo,
): number | null {
  if (!price || !Number.isFinite(price.usd) || price.usd < 0) {
    return null;
  }

  const amount = Number(balance.balance);

  if (!Number.isFinite(amount) || amount < 0) {
    return null;
  }

  const value = amount * price.usd;

  return Number.isFinite(value) ? value : null;
}

function AssetRow({
  asset,
  price,
}: {
  asset: AssetBalance;
  price?: PriceInfo;
}) {
  const value = assetValueUsd(asset, price);
  const change = Number.isFinite(price?.change24h)
    ? price!.change24h!
    : null;
  const up = change !== null && change >= 0;

  return (
    <div className="flex items-center justify-between gap-3 rounded-xl border border-[#1e293b] bg-[#0F1621] px-4 py-3">
      <div className="flex min-w-0 items-center gap-3">
        <div
          aria-hidden="true"
          className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-[#33C5E0]/10 text-sm font-bold text-[#33C5E0]"
        >
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

      <div className="shrink-0 text-right">
        {value !== null ? (
          <p className="text-sm font-semibold text-slate-100">
            {usd.format(value)}
          </p>
        ) : (
          <p className="text-xs text-slate-500">No price available</p>
        )}

        {change !== null && (
          <p
            className={cn(
              "flex items-center justify-end gap-0.5 text-xs font-medium",
              up ? "text-emerald-400" : "text-red-400",
            )}
            aria-label={`24-hour price change: ${up ? "up" : "down"} ${Math.abs(change).toFixed(2)} percent`}
          >
            {up ? (
              <TrendingUp aria-hidden="true" className="h-3 w-3" />
            ) : (
              <TrendingDown aria-hidden="true" className="h-3 w-3" />
            )}
            {Math.abs(change).toFixed(2)}%
          </p>
        )}
      </div>
    </div>
  );
}

/**
 * Multi-asset wallet balance card with USD price feeds.
 *
 * Balances come from WalletContext. Prices are supplied by useTokenPrices.
 * Assets without a valid price are excluded from the displayed portfolio
 * valuation rather than being assigned an estimated value.
 */
export function WalletBalanceCard() {
  const {
    isConnected,
    address,
    networkName,
    selectedWalletName,
    balances,
    balancesLoading,
    balancesError,
    refreshBalances,
  } = useWallet();

  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [refreshingBalances, setRefreshingBalances] = useState(false);

  const symbols = useMemo(
    () => [...new Set(balances.map((balance) => balance.symbol))],
    [balances],
  );

  const {
    prices,
    loading: pricesLoading,
    error: pricesError,
    lastUpdated,
  } = useTokenPrices(symbols);

  const totalUsd = useMemo(
    () =>
      balances.reduce((sum, balance) => {
        const value = assetValueUsd(balance, prices[balance.symbol]);

        return value !== null ? sum + value : sum;
      }, 0),
    [balances, prices],
  );

  const unpricedAssetCount = useMemo(
    () =>
      balances.filter(
        (balance) => assetValueUsd(balance, prices[balance.symbol]) === null,
      ).length,
    [balances, prices],
  );

  if (!isConnected) {
    return null;
  }

  const refreshing =
    balancesLoading || pricesLoading || refreshingBalances;

  const handleRefresh = async () => {
    if (refreshingBalances) {
      return;
    }

    setRefreshingBalances(true);
    setRefreshError(null);

    try {
      await refreshBalances();
    } catch (error) {
      setRefreshError(
        error instanceof Error
          ? error.message
          : "Unable to refresh wallet balances. Please try again.",
      );
    } finally {
      setRefreshingBalances(false);
    }
  };

  return (
    <section
      aria-labelledby="wallet-balances-heading"
      className="w-full rounded-2xl border border-[#1e293b] bg-[#0B111A] p-5"
    >
      <div className="mb-4 flex items-start justify-between gap-3">
        <div className="flex items-center gap-2">
          <Wallet aria-hidden="true" className="h-5 w-5 text-[#33C5E0]" />

          <div>
            <h3
              id="wallet-balances-heading"
              className="text-sm font-semibold text-slate-100"
            >
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
          type="button"
          onClick={handleRefresh}
          disabled={refreshingBalances}
          className="rounded-lg border border-[#1e293b] p-2 text-slate-400 transition-colors hover:border-[#33C5E0]/50 hover:text-[#33C5E0] disabled:cursor-not-allowed disabled:opacity-50"
          aria-label="Refresh balances"
          aria-busy={refreshingBalances}
        >
          <RefreshCw
            aria-hidden="true"
            className={cn("h-4 w-4", refreshing && "animate-spin")}
          />
        </button>
      </div>

      <div className="mb-4 grid gap-3 rounded-xl border border-[#1e293b] bg-[#0F1621] px-4 py-3 text-xs text-slate-400 sm:grid-cols-3">
        <div className="min-w-0">
          <p className="mb-1 uppercase tracking-wide text-slate-500">
            Account
          </p>
          <p
            className="truncate font-mono text-slate-100"
            title={address ?? undefined}
          >
            {address || "Unavailable"}
          </p>
        </div>

        <div className="min-w-0">
          <p className="mb-1 uppercase tracking-wide text-slate-500">
            Network
          </p>
          <p className="truncate font-semibold text-[#33C5E0]">
            {networkName || "Unknown"}
          </p>
        </div>

        <div className="min-w-0">
          <p className="mb-1 uppercase tracking-wide text-slate-500">
            Wallet
          </p>
          <p className="truncate font-semibold text-slate-100">
            {selectedWalletName || "Connected wallet"}
          </p>
        </div>
      </div>

      <div className="mb-4 rounded-xl bg-gradient-to-br from-[#33C5E0]/10 to-transparent px-4 py-3">
        <p className="text-xs uppercase tracking-wide text-slate-400">
          Total Value
        </p>

        <p
          className="text-2xl font-bold text-slate-50"
          aria-live="polite"
          aria-atomic="true"
        >
          {usd.format(totalUsd)}
        </p>

        {unpricedAssetCount > 0 && (
          <p className="mt-1 text-xs text-amber-400">
            {unpricedAssetCount}{" "}
            {unpricedAssetCount === 1 ? "asset has" : "assets have"} no valid
            price and {unpricedAssetCount === 1 ? "is" : "are"} excluded from
            this total.
          </p>
        )}
      </div>

      {(balancesError || refreshError) && (
        <div
          role="alert"
          className="mb-3 flex items-start gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-400"
        >
          <AlertCircle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{refreshError || balancesError}</span>
        </div>
      )}

      {!balancesError && !refreshError && pricesError && (
        <div
          role="status"
          className="mb-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-400"
        >
          Prices unavailable: {pricesError}
        </div>
      )}

      <div className="space-y-2">
        <AnimatePresence initial={false}>
          {balancesLoading && balances.length === 0 && (
            <motion.div
              key="balance-skeleton"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="space-y-2"
              aria-label="Loading wallet balances"
              role="status"
            >
              {[0, 1, 2].map((index) => (
                <div
                  key={index}
                  className="h-16 animate-pulse rounded-xl border border-[#1e293b] bg-[#0F1621]"
                />
              ))}
            </motion.div>
          )}

          {!balancesLoading &&
            balances.length === 0 &&
            !balancesError && (
              <motion.p
                key="empty-balances"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="py-6 text-center text-sm text-slate-500"
              >
                No assets held on this account.
              </motion.p>
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
    </section>
  );
}

export default WalletBalanceCard;