"use client";

import React, { createContext, useContext, useEffect, useState } from "react";

export type NetworkId = "mainnet" | "testnet" | "futurenet" | "localhost";

export interface NetworkConfig {
  id: NetworkId;
  name: string;
  shortName: string;
  rpcUrl: string;
  horizonUrl: string;
  networkPassphrase: string;
  badgeColor: string;
  badgeBg: string;
  badgeBorder: string;
  badgeText: string;
  dotColor: string;
  defaultContractId: string;
}

export const NETWORKS: Record<NetworkId, NetworkConfig> = {
  mainnet: {
    id: "mainnet",
    name: "Mainnet (Public)",
    shortName: "Mainnet",
    rpcUrl: "https://rpc.mainnet.stellar.org",
    horizonUrl: "https://horizon.stellar.org",
    networkPassphrase: "Public Global Stellar Network ; September 2015",
    badgeColor: "emerald",
    badgeBg: "bg-emerald-500/10",
    badgeBorder: "border-emerald-500/30",
    badgeText: "text-emerald-400",
    dotColor: "bg-emerald-400",
    defaultContractId: "CCW67TSBZVTZPT2PP5FE62EBY6GL3KCQSXYGLF73264P3EGBB3LCGN4P",
  },
  testnet: {
    id: "testnet",
    name: "Testnet",
    shortName: "Testnet",
    rpcUrl: "https://soroban-testnet.stellar.org",
    horizonUrl: "https://horizon-testnet.stellar.org",
    networkPassphrase: "Test SDF Network ; September 2015",
    badgeColor: "cyan",
    badgeBg: "bg-cyan-500/10",
    badgeBorder: "border-cyan-500/30",
    badgeText: "text-cyan-400",
    dotColor: "bg-cyan-400",
    defaultContractId: "CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q",
  },
  futurenet: {
    id: "futurenet",
    name: "Futurenet",
    shortName: "Futurenet",
    rpcUrl: "https://rpc-futurenet.stellar.org",
    horizonUrl: "https://horizon-futurenet.stellar.org",
    networkPassphrase: "Test SDF Future Network ; October 2022",
    badgeColor: "purple",
    badgeBg: "bg-purple-500/10",
    badgeBorder: "border-purple-500/30",
    badgeText: "text-purple-400",
    dotColor: "bg-purple-400",
    defaultContractId: "CB6EAKZTWL67L7B55W4D337FUGJ36J2G4T7N5K6V6P6N7Q8R9S0T1U2V",
  },
  localhost: {
    id: "localhost",
    name: "Localhost (Standalone)",
    shortName: "Localhost",
    rpcUrl: "http://localhost:8000/soroban/rpc",
    horizonUrl: "http://localhost:8000",
    networkPassphrase: "Standalone Network ; February 2017",
    badgeColor: "amber",
    badgeBg: "bg-amber-500/10",
    badgeBorder: "border-amber-500/30",
    badgeText: "text-amber-400",
    dotColor: "bg-amber-400",
    defaultContractId: "CDLZFC3SYJYDVR7P6CC4D5RUDTFGWXMGAW5W6L2G4T7N5K6V6P6N7Q8R",
  },
};

export const ALL_NETWORKS = Object.values(NETWORKS);

interface NetworkContextType {
  network: NetworkConfig;
  networkId: NetworkId;
  setNetworkId: (id: NetworkId) => void;
  networks: NetworkConfig[];
}

const NetworkContext = createContext<NetworkContextType | undefined>(undefined);

export const useNetwork = () => {
  const context = useContext(NetworkContext);
  if (!context) {
    throw new Error("useNetwork must be used within a NetworkProvider");
  }
  return context;
};

export const STORAGE_KEY = "soroscope_selected_network";

export const NetworkProvider = ({ children }: { children: React.ReactNode }) => {
  const [networkId, setNetworkIdState] = useState<NetworkId>("testnet");

  useEffect(() => {
    try {
      const savedNetwork = localStorage.getItem(STORAGE_KEY) as NetworkId | null;
      if (savedNetwork && NETWORKS[savedNetwork]) {
        setNetworkIdState(savedNetwork);
      }
    } catch (e) {
      console.warn("Failed to load saved network preferences", e);
    }
  }, []);

  const setNetworkId = (id: NetworkId) => {
    if (!NETWORKS[id]) return;
    setNetworkIdState(id);
    try {
      localStorage.setItem(STORAGE_KEY, id);
    } catch (e) {
      console.warn("Failed to persist network preference", e);
    }
  };

  const currentNetwork = NETWORKS[networkId] || NETWORKS.testnet;

  return (
    <NetworkContext.Provider
      value={{
        network: currentNetwork,
        networkId,
        setNetworkId,
        networks: ALL_NETWORKS,
      }}
    >
      {children}
    </NetworkContext.Provider>
  );
};
