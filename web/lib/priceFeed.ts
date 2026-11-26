// Real-time USD price feed for wallet assets.
//
// Prices are sourced from a public REST endpoint keyed by a small map of
// Stellar asset symbols to feed identifiers. The feed is intentionally
// pluggable: point `PRICE_FEED_URL` at any endpoint that returns a
// `{ <id>: { usd: number } }` shape (the CoinGecko `simple/price` schema),
// or override `fetchPrices` in tests.

export interface PriceInfo {
  /** Latest USD price. */
  usd: number;
  /** 24h percentage change, when the feed provides it. */
  change24h: number | null;
  /** Epoch millis when this quote was fetched. */
  fetchedAt: number;
}

export type PriceMap = Record<string, PriceInfo>;

const PRICE_FEED_URL =
  process.env.NEXT_PUBLIC_PRICE_FEED_URL ??
  "https://api.coingecko.com/api/v3/simple/price";

// Maps on-chain asset symbols to price-feed coin ids. Assets not listed here
// have no USD quote and render without a fiat value.
const SYMBOL_TO_FEED_ID: Record<string, string> = {
  XLM: "stellar",
  USDC: "usd-coin",
  USDT: "tether",
  BTC: "bitcoin",
  ETH: "ethereum",
  AQUA: "aquarius",
  yXLM: "stellar",
};

/**
 * Fetch USD prices for the given asset symbols. Unknown symbols are skipped.
 * Returns a map keyed by the original symbol so callers can look prices up
 * directly by `AssetBalance.symbol`.
 */
export async function fetchPrices(
  symbols: string[],
  signal?: AbortSignal,
): Promise<PriceMap> {
  const wanted = Array.from(new Set(symbols)).filter(
    (s) => SYMBOL_TO_FEED_ID[s],
  );
  if (wanted.length === 0) return {};

  const ids = Array.from(new Set(wanted.map((s) => SYMBOL_TO_FEED_ID[s])));
  const params = new URLSearchParams({
    ids: ids.join(","),
    vs_currencies: "usd",
    include_24hr_change: "true",
  });

  const res = await fetch(`${PRICE_FEED_URL}?${params.toString()}`, {
    signal,
    headers: { Accept: "application/json" },
  });
  if (!res.ok) {
    throw new Error(`Price feed request failed (${res.status})`);
  }

  const raw = (await res.json()) as Record<
    string,
    { usd?: number; usd_24h_change?: number }
  >;
  const fetchedAt = Date.now();

  const out: PriceMap = {};
  for (const symbol of wanted) {
    const feedId = SYMBOL_TO_FEED_ID[symbol];
    const quote = raw[feedId];
    if (quote && typeof quote.usd === "number") {
      out[symbol] = {
        usd: quote.usd,
        change24h:
          typeof quote.usd_24h_change === "number"
            ? quote.usd_24h_change
            : null,
        fetchedAt,
      };
    }
  }
  return out;
}
