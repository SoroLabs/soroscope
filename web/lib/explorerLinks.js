/**
 * Stellar explorer link helpers (issue #842).
 *
 * SoroScope runs against several networks and two different explorer
 * front-ends. Every "view on explorer" link in the UI needs the same three
 * things: a display-safe shortened identifier, the right base URL for the
 * active network, and a fully-qualified resource path.
 *
 * This module is intentionally framework-free (plain CJS + a `.d.ts`) so it can
 * be unit tested with `node --test` and reused from any component without
 * pulling in React or the network context.
 */

/** Number of leading characters kept when shortening an identifier. */
const SHORTEN_PREFIX_LENGTH = 4;

/** Number of trailing characters kept when shortening an identifier. */
const SHORTEN_SUFFIX_LENGTH = 4;

/** Separator inserted between the kept prefix and suffix. */
const SHORTEN_SEPARATOR = '...';

/**
 * Stellar Expert path segment per SoroScope network id. Note that mainnet is
 * exposed as `public` by stellar.expert, which is not the same as the
 * `mainnet` name used by Horizon/RPC.
 */
const STELLAR_EXPERT_NETWORK_SEGMENTS = {
  mainnet: 'public',
  testnet: 'testnet',
  futurenet: 'futurenet',
};

/**
 * Soroban Lab explorer origins per SoroScope network id. These are the
 * network-specific Soroban RPC hosts, which also serve the contract explorer.
 */
const SOROBAN_EXPLORER_ORIGINS = {
  mainnet: 'https://soroban.stellar.org',
  testnet: 'https://soroban-testnet.stellar.org',
  futurenet: 'https://soroban-futurenet.stellar.org',
};

/** Explorer providers supported by {@link getExplorerBaseUrl}. */
const EXPLORER_PROVIDERS = {
  stellarExpert: 'stellarExpert',
  soroban: 'soroban',
};

/**
 * Normalize a network id to one the explorer maps know about. Unknown or
 * missing ids fall back to `testnet`, matching the default in NetworkContext.
 *
 * @param {unknown} networkId
 * @returns {string}
 */
function resolveNetworkId(networkId) {
  if (typeof networkId !== 'string') return 'testnet';
  const normalized = networkId.trim().toLowerCase();
  if (normalized === 'mainnet' || normalized === 'public') return 'mainnet';
  if (
    normalized === 'testnet' ||
    normalized === 'futurenet' ||
    normalized === 'localhost'
  ) {
    return normalized;
  }
  return 'testnet';
}

/**
 * Shorten a Stellar identifier for display, e.g. `CC3J...4KL9`.
 *
 * Values that are already at or below the combined kept length are returned
 * untouched so short labels are never mangled into something unreadable.
 *
 * @param {unknown} value
 * @param {{ prefixLength?: number, suffixLength?: number, separator?: string }} [options]
 * @returns {string}
 */
function shortenAddress(value, options) {
  if (typeof value !== 'string') return '';
  const trimmed = value.trim();
  if (trimmed.length === 0) return '';

  const prefixLength = options?.prefixLength ?? SHORTEN_PREFIX_LENGTH;
  const suffixLength = options?.suffixLength ?? SHORTEN_SUFFIX_LENGTH;
  const separator = options?.separator ?? SHORTEN_SEPARATOR;

  if (prefixLength <= 0 || suffixLength <= 0) return trimmed;
  if (trimmed.length <= prefixLength + suffixLength) return trimmed;

  return (
    trimmed.slice(0, prefixLength) + separator + trimmed.slice(-suffixLength)
  );
}

/**
 * True when the value can be linked to (non-empty, url-safe, no whitespace).
 *
 * @param {unknown} value
 * @returns {boolean}
 */
function isLinkableIdentifier(value) {
  return (
    typeof value === 'string' &&
    value.trim().length > 0 &&
    !/\s/.test(value.trim())
  );
}

/**
 * Percent-encode an identifier so it is safe inside a URL path segment.
 *
 * @param {unknown} value
 * @returns {string}
 */
function encodeIdentifier(value) {
  if (typeof value !== 'string') return '';
  return encodeURIComponent(value.trim());
}

/**
 * Base URL for the requested explorer on the requested network.
 *
 * `localhost` has no public explorer, so it resolves to `null` and callers
 * should render a non-linked label instead of a dead link.
 *
 * @param {string} networkId
 * @param {string} [provider] One of `EXPLORER_PROVIDERS`.
 * @returns {string | null}
 */
function getExplorerBaseUrl(networkId, provider = EXPLORER_PROVIDERS.stellarExpert) {
  const id = resolveNetworkId(networkId);
  if (id === 'localhost') return null;

  if (provider === EXPLORER_PROVIDERS.soroban) {
    return SOROBAN_EXPLORER_ORIGINS[id] ?? null;
  }

  const segment = STELLAR_EXPERT_NETWORK_SEGMENTS[id];
  return segment ? `https://stellar.expert/explorer/${segment}` : null;
}

/**
 * Build a full explorer URL for an entity on the active network.
 *
 * @param {string} networkId
 * @param {'contract' | 'account' | 'tx'} kind
 * @param {unknown} identifier
 * @param {string} [provider] One of `EXPLORER_PROVIDERS`.
 * @returns {string | null} `null` when the identifier or network cannot be linked.
 */
function buildExplorerUrl(networkId, kind, identifier, provider = EXPLORER_PROVIDERS.stellarExpert) {
  if (!isLinkableIdentifier(identifier)) return null;

  const base = getExplorerBaseUrl(networkId, provider);
  if (!base) return null;

  const encoded = encodeIdentifier(identifier);

  if (provider === EXPLORER_PROVIDERS.soroban) {
    // The Soroban Lab explorer only hosts contracts.
    if (kind !== 'contract') return null;
    return `${base}/contract/${encoded}`;
  }

  if (kind === 'contract') return `${base}/contract/${encoded}`;
  if (kind === 'account') return `${base}/account/${encoded}`;
  if (kind === 'tx') return `${base}/tx/${encoded}`;
  return null;
}

/**
 * Explorer URL for a Soroban contract address.
 *
 * @param {string} networkId
 * @param {unknown} contractId
 * @param {string} [provider]
 * @returns {string | null}
 */
function buildContractExplorerUrl(networkId, contractId, provider = EXPLORER_PROVIDERS.stellarExpert) {
  return buildExplorerUrl(networkId, 'contract', contractId, provider);
}

/**
 * Explorer URL for a Stellar account.
 *
 * @param {string} networkId
 * @param {unknown} address
 * @param {string} [provider]
 * @returns {string | null}
 */
function buildAccountExplorerUrl(networkId, address, provider = EXPLORER_PROVIDERS.stellarExpert) {
  return buildExplorerUrl(networkId, 'account', address, provider);
}

/**
 * Explorer URL for a transaction hash.
 *
 * @param {string} networkId
 * @param {unknown} hash
 * @param {string} [provider]
 * @returns {string | null}
 */
function buildTxExplorerUrl(networkId, hash, provider = EXPLORER_PROVIDERS.stellarExpert) {
  return buildExplorerUrl(networkId, 'tx', hash, provider);
}

/**
 * Attributes required to open a link safely in a new browsing context.
 * Exported so tests and consumers share a single source of truth.
 */
const SAFE_EXTERNAL_LINK_PROPS = {
  target: '_blank',
  rel: 'noopener noreferrer',
};

module.exports = {
  SHORTEN_PREFIX_LENGTH,
  SHORTEN_SUFFIX_LENGTH,
  SHORTEN_SEPARATOR,
  STELLAR_EXPERT_NETWORK_SEGMENTS,
  SOROBAN_EXPLORER_ORIGINS,
  EXPLORER_PROVIDERS,
  SAFE_EXTERNAL_LINK_PROPS,
  resolveNetworkId,
  shortenAddress,
  isLinkableIdentifier,
  encodeIdentifier,
  getExplorerBaseUrl,
  buildExplorerUrl,
  buildContractExplorerUrl,
  buildAccountExplorerUrl,
  buildTxExplorerUrl,
};
