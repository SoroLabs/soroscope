export type ExplorerProvider = "stellarExpert" | "soroban";

export type ExplorerEntityKind = "contract" | "account" | "tx";

export interface ShortenAddressOptions {
  prefixLength?: number;
  suffixLength?: number;
  separator?: string;
}

/** Number of leading characters kept when shortening an identifier. */
export declare const SHORTEN_PREFIX_LENGTH: number;

/** Number of trailing characters kept when shortening an identifier. */
export declare const SHORTEN_SUFFIX_LENGTH: number;

/** Separator inserted between the kept prefix and suffix. */
export declare const SHORTEN_SEPARATOR: string;

/** Stellar Expert path segment per SoroScope network id. */
export declare const STELLAR_EXPERT_NETWORK_SEGMENTS: Record<string, string>;

/** Soroban Lab explorer origins per SoroScope network id. */
export declare const SOROBAN_EXPLORER_ORIGINS: Record<string, string>;

/** Explorer providers supported by `getExplorerBaseUrl`. */
export declare const EXPLORER_PROVIDERS: {
  stellarExpert: "stellarExpert";
  soroban: "soroban";
};

/** Attributes required to open a link safely in a new browsing context. */
export declare const SAFE_EXTERNAL_LINK_PROPS: {
  target: "_blank";
  rel: string;
};

/** Normalize a network id to one the explorer maps know about. */
export declare function resolveNetworkId(networkId: unknown): string;

/** Shorten a Stellar identifier for display, e.g. `CC3J...4KL9`. */
export declare function shortenAddress(
  value: unknown,
  options?: ShortenAddressOptions,
): string;

/** True when the value can be linked to (non-empty, url-safe, no whitespace). */
export declare function isLinkableIdentifier(value: unknown): boolean;

/** Percent-encode an identifier so it is safe inside a URL path segment. */
export declare function encodeIdentifier(value: unknown): string;

/** Base URL for the requested explorer on the requested network. */
export declare function getExplorerBaseUrl(
  networkId: string,
  provider?: ExplorerProvider,
): string | null;

/** Build a full explorer URL for an entity on the active network. */
export declare function buildExplorerUrl(
  networkId: string,
  kind: ExplorerEntityKind,
  identifier: unknown,
  provider?: ExplorerProvider,
): string | null;

/** Explorer URL for a Soroban contract address. */
export declare function buildContractExplorerUrl(
  networkId: string,
  contractId: unknown,
  provider?: ExplorerProvider,
): string | null;

/** Explorer URL for a Stellar account. */
export declare function buildAccountExplorerUrl(
  networkId: string,
  address: unknown,
  provider?: ExplorerProvider,
): string | null;

/** Explorer URL for a transaction hash. */
export declare function buildTxExplorerUrl(
  networkId: string,
  hash: unknown,
  provider?: ExplorerProvider,
): string | null;
