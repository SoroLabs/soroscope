"use client";
import { useState } from "react";

// Minimal wallet store (Zustand-compatible shape without the dependency).
type WalletState = { publicKey: string | null };
const listeners = new Set<() => void>();
let state: WalletState = { publicKey: null };
function setPublicKey(publicKey: string | null) {
  state = { publicKey };
  listeners.forEach((l) => l());
}
export function useWalletStore(): WalletState & {
  setPublicKey: typeof setPublicKey;
  disconnect: () => void;
} {
  const [, force] = useState(0);
  useState(() => {
    const l = () => force((n) => n + 1);
    listeners.add(l);
    return undefined;
  });
  return {
    publicKey: state.publicKey,
    setPublicKey,
    disconnect: () => setPublicKey(null),
  };
}
export const truncateKey = (k: string) => `${k.slice(0, 4)}…${k.slice(-4)}`;

export async function connectFreighter(): Promise<string> {
  const mod = await import("@stellar/freighter-api");
  const key =
    (await (mod as { getPublicKey?: () => Promise<string> }).getPublicKey?.()) ??
    (await (mod as { default?: { getPublicKey?: () => Promise<string> } }).default?.getPublicKey?.()) ??
    "";
  if (!key) throw new Error("Freighter not available");
  setPublicKey(key);
  return key;
}

export function connectAlbedo(): void {
  const params = new URLSearchParams({
    redirect: typeof window !== "undefined" ? window.location.href : "",
  });
  window.location.href = `https://albedo.link/?${params.toString()}`;
}

export default function WalletConnectModal({
  onClose,
}: {
  onClose?: () => void;
}) {
  const { publicKey, disconnect } = useWalletStore();
  const [error, setError] = useState<string | null>(null);

  const handleFreighter = async () => {
    setError(null);
    try {
      await connectFreighter();
    } catch {
      setError("Freighter not found. Please install the Freighter extension.");
    }
  };

  if (publicKey) {
    return (
      <div data-testid="wallet-modal">
        <span data-testid="wallet-pubkey">{truncateKey(publicKey)}</span>
        <button
          onClick={() => {
            disconnect();
            onClose?.();
          }}
        >
          Disconnect
        </button>
      </div>
    );
  }

  return (
    <div data-testid="wallet-modal">
      <button onClick={handleFreighter}>Connect Freighter</button>
      <button onClick={connectAlbedo}>Connect Albedo</button>
      {error && <p role="alert">{error}</p>}
    </div>
  );
}
