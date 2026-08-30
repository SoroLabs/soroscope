// Fetches native + issued-asset balances for a Stellar account from Horizon.
//
// Horizon exposes account balances at `GET /accounts/{account_id}`. The
// `balances` array contains one entry per asset the account holds (plus the
// native XLM balance and, on some accounts, liquidity-pool shares which we
// filter out since they are not spendable token balances).

export interface AssetBalance {
  /** Stable key used for React lists and price lookups, e.g. "XLM" or "USDC:GA5Z…". */
  id: string;
  /** Display symbol, e.g. "XLM", "USDC". */
  symbol: string;
  /** Asset issuer account, or null for the native asset. */
  issuer: string | null;
  /** Human-readable balance (Horizon already returns this as a decimal string). */
  balance: string;
  /** Whether this is the native XLM asset. */
  isNative: boolean;
}

interface HorizonBalanceLine {
  asset_type: string;
  asset_code?: string;
  asset_issuer?: string;
  balance: string;
}

interface HorizonAccountResponse {
  balances?: HorizonBalanceLine[];
}

/**
 * Fetch the spendable asset balances for `address` from the given Horizon URL.
 *
 * @throws if the account is not found (404) or the request otherwise fails, so
 *         callers can surface a meaningful error instead of showing stale data.
 */
export async function fetchAccountBalances(
  horizonUrl: string,
  address: string,
  signal?: AbortSignal,
): Promise<AssetBalance[]> {
  const base = horizonUrl.replace(/\/+$/, "");
  const res = await fetch(`${base}/accounts/${address}`, {
    signal,
    headers: { Accept: "application/json" },
  });

  if (res.status === 404) {
    // Unfunded account — no trustlines yet. Treat as an empty balance set
    // rather than an error so a freshly created wallet renders cleanly.
    return [];
  }

  if (!res.ok) {
    throw new Error(`Horizon request failed (${res.status})`);
  }

  const data = (await res.json()) as HorizonAccountResponse;
  const lines = data.balances ?? [];

  return lines
    .filter((line) => line.asset_type !== "liquidity_pool_shares")
    .map((line) => {
      const isNative = line.asset_type === "native";
      const symbol = isNative ? "XLM" : line.asset_code ?? "UNKNOWN";
      const issuer = isNative ? null : line.asset_issuer ?? null;
      const id = isNative ? "XLM" : `${symbol}:${issuer ?? ""}`;
      return { id, symbol, issuer, balance: line.balance, isNative };
    });
}
