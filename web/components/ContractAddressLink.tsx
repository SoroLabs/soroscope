import React from "react";
import { ExternalLink } from "lucide-react";
import { useOptionalNetwork } from "../context/NetworkContext";
import {
  buildAccountExplorerUrl,
  buildContractExplorerUrl,
  buildTxExplorerUrl,
  EXPLORER_PROVIDERS,
  isLinkableIdentifier,
  SAFE_EXTERNAL_LINK_PROPS,
  shortenAddress,
  type ExplorerProvider,
} from "../lib/explorerLinks";

export interface ContractAddressLinkProps {
  /** Stellar strkey to display and link (contract, account, or tx hash). */
  value: string;
  /** Which explorer route the value maps to. Defaults to `contract`. */
  kind?: "contract" | "account" | "tx";
  /**
   * Explorer front-end. Defaults to `stellarExpert`; `soroban` uses the
   * network-specific Soroban Lab explorer.
   */
  provider?: ExplorerProvider;
  /**
   * Network id override. Defaults to the active network from
   * `NetworkContext`, so the link always follows the selected network.
   */
  networkId?: string;
  /** Label rendered before the address. Set to `null` to hide it. */
  label?: React.ReactNode;
  /** Override the shortened address text. */
  children?: React.ReactNode;
  /** Number of leading characters kept when shortening. */
  prefixLength?: number;
  /** Number of trailing characters kept when shortening. */
  suffixLength?: number;
  showIcon?: boolean;
  className?: string;
}

/**
 * Renders a shortened Stellar identifier that links to the block explorer for
 * the active network. Falls back to plain (non-linked) shortened text when the
 * value or the network has no explorer (e.g. an empty input or `localhost`),
 * so the UI never renders a dead link.
 *
 * Opens in a new tab with `rel="noopener noreferrer"`.
 */
export function ContractAddressLink({
  value,
  kind = "contract",
  provider = EXPLORER_PROVIDERS.stellarExpert,
  networkId,
  label = "Contract ID",
  children,
  prefixLength,
  suffixLength,
  showIcon = true,
  className = "",
}: ContractAddressLinkProps) {
  const network = useOptionalNetwork();
  const resolvedNetworkId = networkId ?? network?.networkId ?? "testnet";

  const displayText = children ?? shortenAddress(value, { prefixLength, suffixLength });

  const href = isLinkableIdentifier(value)
    ? kind === "account"
      ? buildAccountExplorerUrl(resolvedNetworkId, value, provider)
      : kind === "tx"
        ? buildTxExplorerUrl(resolvedNetworkId, value, provider)
        : buildContractExplorerUrl(resolvedNetworkId, value, provider)
    : null;

  const content = (
    <>
      {label ? <span className="text-slate-400">{label}: </span> : null}
      <span className="font-mono">{displayText}</span>
      {showIcon && href ? (
        <ExternalLink className="inline-block h-3 w-3 shrink-0 align-middle" aria-hidden="true" />
      ) : null}
    </>
  );

  if (!href) {
    return (
      <span
        className={`inline-flex items-center gap-1 text-sm text-slate-300 ${className}`}
        title={typeof value === "string" ? value : undefined}
      >
        {content}
      </span>
    );
  }

  return (
    <a
      href={href}
      {...SAFE_EXTERNAL_LINK_PROPS}
      title={`View on explorer: ${String(value).trim()}`}
      className={`inline-flex items-center gap-1 font-mono text-sm text-cyan-400 transition-colors hover:text-cyan-300 hover:underline focus:outline-none focus:ring-2 focus:ring-cyan-500/50 ${className}`}
    >
      {content}
    </a>
  );
}

export default ContractAddressLink;
