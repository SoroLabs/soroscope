"use client";

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { useNetwork } from "./NetworkContext";
import { fetchAccountBalances, AssetBalance } from "../lib/stellarBalances";

interface WalletContextType {
  connect: (moduleId: string) => Promise<void>;
  disconnect: () => Promise<void>;
  address: string | null;
  isConnected: boolean;
  isConnecting: boolean;
  selectedWalletId: string | null;
  openModal: () => void;
  closeModal: () => void;
  isModalOpen: boolean;
  supportedWallets: { id: string; name: string; icon: string }[];
  error: string | null;
  balances: AssetBalance[];
  balancesLoading: boolean;
  balancesError: string | null;
  refreshBalances: () => void;
}

const WalletContext = createContext<WalletContextType | undefined>(undefined);

export const useWallet = () => {
  const context = useContext(WalletContext);
  if (!context) {
    throw new Error("useWallet must be used within a WalletProvider");
  }
  return context;
};

export const WalletProvider = ({ children }: { children: React.ReactNode }) => {
  const { networkId, network } = useNetwork();
  const [address, setAddress] = useState<string | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [selectedWalletId, setSelectedWalletId] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [kit, setKit] = useState<any>(null);

  // Account balance state lives alongside the wallet so it can be cleared
  // atomically on disconnect / account change and never leak between sessions.
  const [balances, setBalances] = useState<AssetBalance[]>([]);
  const [balancesLoading, setBalancesLoading] = useState(false);
  const [balancesError, setBalancesError] = useState<string | null>(null);
  const balanceReqRef = useRef<AbortController | null>(null);
  const [balanceRefreshTick, setBalanceRefreshTick] = useState(0);

  // Drops every piece of derived, address-scoped state. Called on disconnect
  // and whenever the active account changes so no stale balance is shown for
  // the wrong account.
  const clearWalletState = useCallback(() => {
    balanceReqRef.current?.abort();
    balanceReqRef.current = null;
    setBalances([]);
    setBalancesLoading(false);
    setBalancesError(null);
  }, []);

  useEffect(() => {
    const initKit = async () => {
      try {
        const walletKitModule = await import("@creit.tech/stellar-wallets-kit");

        let walletNet = walletKitModule.WalletNetwork.TESTNET;
        if (networkId === "mainnet") walletNet = walletKitModule.WalletNetwork.PUBLIC;
        else if (networkId === "futurenet") walletNet = walletKitModule.WalletNetwork.FUTURENET;
        else if (networkId === "localhost") walletNet = walletKitModule.WalletNetwork.SANDBOX || walletKitModule.WalletNetwork.TESTNET;

        const kitInstance = new walletKitModule.StellarWalletsKit({
          network: walletNet,
          selectedWalletId: walletKitModule.FREIGHTER_ID,
          modules: walletKitModule.allowAllModules(),
        });

        setKit(kitInstance);

        const savedAddress = localStorage.getItem("inheritx_wallet_address");
        const savedWalletId = localStorage.getItem("inheritx_wallet_id");
        if (savedAddress && savedWalletId) {
          setAddress(savedAddress);
          setSelectedWalletId(savedWalletId);
        }
      } catch (err) {
        console.error("Failed to initialize wallet kit:", err);
        setError("Failed to load wallet kit");
      }
    };

    initKit();
  }, [networkId]);

  // Fetch balances whenever the connected account, network, or an explicit
  // refresh changes. Stale balances are cleared up-front so the UI never shows
  // one account's holdings against another's address while a fetch is in flight.
  useEffect(() => {
    if (!address) {
      clearWalletState();
      return;
    }

    balanceReqRef.current?.abort();
    const controller = new AbortController();
    balanceReqRef.current = controller;

    setBalances([]);
    setBalancesLoading(true);
    setBalancesError(null);

    fetchAccountBalances(network.horizonUrl, address, controller.signal)
      .then((result) => {
        if (controller.signal.aborted) return;
        setBalances(result);
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted) return;
        setBalancesError(
          err instanceof Error ? err.message : "Failed to load balances",
        );
      })
      .finally(() => {
        if (controller.signal.aborted) return;
        setBalancesLoading(false);
      });

    return () => controller.abort();
  }, [address, network.horizonUrl, balanceRefreshTick, clearWalletState]);

  const refreshBalances = useCallback(() => {
    setBalanceRefreshTick((t) => t + 1);
  }, []);

  const supportedWallets = [
    { id: "freighter", name: "Freighter", icon: "https://stellar.creit.tech/wallet-icons/freighter.png" },
    { id: "albedo", name: "Albedo", icon: "https://stellar.creit.tech/wallet-icons/albedo.png" },
    { id: "xbull", name: "xBull", icon: "https://stellar.creit.tech/wallet-icons/xbull.png" },
    { id: "rabet", name: "Rabet", icon: "https://stellar.creit.tech/wallet-icons/rabet.png" },
    { id: "lobstr", name: "Lobstr", icon: "https://stellar.creit.tech/wallet-icons/lobstr.png" },
  ];

  const connectWallet = async (moduleId: string) => {
    if (!kit) {
      setError("Wallet kit not loaded yet");
      return;
    }

    setIsConnecting(true);
    setError(null);

    try {
      kit.setWallet(moduleId);
      const { address: walletAddress } = await kit.getAddress();

      setAddress(walletAddress);
      setSelectedWalletId(moduleId);
      localStorage.setItem("inheritx_wallet_address", walletAddress);
      localStorage.setItem("inheritx_wallet_id", moduleId);
      setIsModalOpen(false);
    } catch (err: any) {
      const errorMessage = err?.message || "Connection failed";
      setError(errorMessage);
      console.error("Wallet connection failed:", err);
    } finally {
      setIsConnecting(false);
    }
  };

  const disconnect = async () => {
    if (kit) {
      try {
        await kit.disconnect();
      } catch (err) {
        console.error("Disconnect error:", err);
      }
    }
    setAddress(null);
    setSelectedWalletId(null);
    setError(null);
    // Drop cached balances and any in-flight balance request so a reconnect
    // (possibly to a different account) never briefly shows the old holdings.
    clearWalletState();
    localStorage.removeItem("inheritx_wallet_address");
    localStorage.removeItem("inheritx_wallet_id");
  };

  const openModal = () => {
    setError(null);
    setIsModalOpen(true);
  };

  const closeModal = () => {
    setError(null);
    setIsModalOpen(false);
  };

  return (
    <WalletContext.Provider
      value={{
        connect: connectWallet,
        disconnect,
        address,
        isConnected: !!address,
        isConnecting,
        selectedWalletId,
        openModal,
        closeModal,
        isModalOpen,
        supportedWallets,
        error,
        balances,
        balancesLoading,
        balancesError,
        refreshBalances,
      }}
    >
      {children}
    </WalletContext.Provider>
  );
};
