import React, { useState, useRef, useEffect } from "react";
import { ChevronDown, Check, Globe, Server } from "lucide-react";
import { useNetwork, NetworkId, NetworkConfig } from "../context/NetworkContext";

interface NetworkSwitcherProps {
  className?: string;
  isMobile?: boolean;
}

export function NetworkSwitcher({ className = "", isMobile = false }: NetworkSwitcherProps) {
  const { network, networkId, setNetworkId, networks } = useNetwork();
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown on click outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Close dropdown on Escape key
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && isOpen) {
        setIsOpen(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen]);

  const handleSelect = (id: NetworkId) => {
    setNetworkId(id);
    setIsOpen(false);
  };

  return (
    <div ref={dropdownRef} className={`relative inline-block text-left ${className}`}>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-label={`Current network: ${network.name}. Click to switch network.`}
        className={`flex min-h-[44px] items-center gap-2 rounded-xl border px-3 py-2 text-xs font-medium transition-all duration-150 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 ${
          isMobile
            ? "w-full justify-between border-slate-800 bg-slate-900 text-slate-200"
            : "border-slate-800 bg-slate-900/90 text-slate-200 hover:bg-slate-800 hover:border-slate-700"
        }`}
      >
        <div className="flex items-center gap-2">
          <span className={`inline-block h-2 w-2 rounded-full ${network.dotColor} animate-pulse`} />
          <span className="font-semibold">{network.shortName}</span>
          <span className={`hidden md:inline-block rounded px-1.5 py-0.5 text-[10px] uppercase font-mono ${network.badgeBg} ${network.badgeText} border ${network.badgeBorder}`}>
            RPC
          </span>
        </div>
        <ChevronDown size={14} className={`text-slate-400 transition-transform duration-200 ${isOpen ? "rotate-180" : ""}`} />
      </button>

      {isOpen && (
        <div
          role="listbox"
          aria-label="Select Stellar Network"
          className={`absolute z-50 mt-2 w-64 rounded-2xl border border-slate-800 bg-slate-900/95 p-1.5 shadow-2xl backdrop-blur-md animate-in fade-in duration-150 ${
            isMobile ? "left-0 right-0 w-full" : "right-0"
          }`}
        >
          <div className="px-3 py-2 border-b border-slate-800/80">
            <div className="flex items-center gap-1.5 text-xs font-semibold text-slate-300">
              <Globe size={13} className="text-cyan-400" />
              <span>Select Environment</span>
            </div>
            <p className="text-[11px] text-slate-400 mt-0.5">
              Updates active RPC endpoints & passphrase
            </p>
          </div>

          <div className="py-1">
            {networks.map((net: NetworkConfig) => {
              const isSelected = net.id === networkId;
              return (
                <button
                  key={net.id}
                  type="button"
                  role="option"
                  aria-selected={isSelected}
                  onClick={() => handleSelect(net.id)}
                  className={`group flex w-full items-center justify-between rounded-xl px-3 py-2.5 text-left text-xs transition-colors ${
                    isSelected
                      ? `${net.badgeBg} text-white font-semibold border ${net.badgeBorder}`
                      : "text-slate-300 hover:bg-slate-800/80 hover:text-white"
                  }`}
                >
                  <div className="flex items-center gap-2.5">
                    <span className={`h-2.5 w-2.5 rounded-full ${net.dotColor} ${isSelected ? "ring-2 ring-offset-1 ring-offset-slate-900 ring-cyan-500" : "opacity-70"}`} />
                    <div>
                      <div className="flex items-center gap-1.5 font-medium">
                        <span>{net.name}</span>
                      </div>
                      <div className="flex items-center gap-1 text-[10px] text-slate-400 font-mono mt-0.5 truncate max-w-[170px]">
                        <Server size={10} className="shrink-0 text-slate-500" />
                        <span className="truncate">{net.rpcUrl.replace("https://", "").replace("http://", "")}</span>
                      </div>
                    </div>
                  </div>
                  {isSelected && <Check size={14} className={net.badgeText} />}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
