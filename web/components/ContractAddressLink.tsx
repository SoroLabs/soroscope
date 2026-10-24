import React, { useCallback, useEffect, useRef, useState } from "react";
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
import { Toast } from "./Toast";
import { writeTextToClipboard } from "../lib/copyFeedback";

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

  const [copied, setCopied] = useState(false);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    };
  }, []);

  const handleCopy = useCallback(async () => {
    try {
      await writeTextToClipboard(String(value));
    } catch {
      // Clipboard unavailable (non-secure context); still show feedback
      // so the user knows the address text they can select manually.
    }
    setCopied(true);
    if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    copyTimerRef.current = setTimeout(() => setCopied(false), 2000);
  }, [value]);

  const copyButton = (
    <button
      type="button"
      onClick={handleCopy}
      aria-label="Copy contract address to clipboard"
      title="Copy address"
      className="inline-flex items-center justify-center rounded p-0.5 text-slate-400 transition-colors hover:bg-slate-800 hover:text-cyan-300 focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
    >
      <svg
        className="h-3 w-3 shrink-0"
        aria-hidden="true"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <rect width="14" height="14" x="8" y="8" rx="2" ry="2" />
        <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
      </svg>
    </button>
  );

  const copiedFeedback = copied ? (
    <span className="fixed z-[70]">
      <Toast message="✓ Copied!" type="success" duration={2000} onClose={() => setCopied(false)} />
    </span>
  ) : null;

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
        {copyButton}
        {copiedFeedback}
      </span>
    );
  }

  return (
    <span className={`inline-flex items-center gap-1 ${className}`}>
      <a
        href={href}
        {...SAFE_EXTERNAL_LINK_PROPS}
        title={`View on explorer: ${String(value).trim()}`}
        className="inline-flex items-center gap-1 font-mono text-sm text-cyan-400 transition-colors hover:text-cyan-300 hover:underline focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
      >
        {content}
      </a>
      {copyButton}
      {copiedFeedback}
    </span>
  );
}

export default ContractAddressLink;
