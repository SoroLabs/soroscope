import Head from "next/head";
import dynamic from "next/dynamic";
import { useRouter } from "next/router";
import React, { useCallback, useEffect, useMemo, useState } from "react";
import { Code, FileCode, ChevronDown, ChevronRight } from "lucide-react";

import { HeaderNav, type NavTab } from "../components/HeaderNav";
import { SEARCH_COMMAND_EVENT } from "../components/GlobalSearchModal";
import { ConnectButton } from "../components/ConnectButton";
import { ResultViewer } from "../components/Resultviewer";
import { InvocationHistory, useInvocationHistory } from "../components/InnovocationHistory";
import { NutritionLabel } from "../components/NutritionLabel";
import { NutritionLabelSkeleton } from "../components/NutritionLabelSkeleton";
import { FunctionSidebar } from "../components/FunctionSidebar";
import { ContractInteraction } from "../components/ContractInteraction";
import { GasGolfingSuggestionsTable } from "../components/GasGolfingSuggestionsTable";
import { GasUsageChart } from "../components/GasUsageChart";
import { ResourceHeatmap } from "../components/ResourceHeatmap";
import { ResultViewerSkeleton } from "../components/ResultViewerSkeleton";
import { FeeEstimationPreview } from "../components/FeeEstimationPreview";
import { SyntaxHighlighter } from "../components/SyntaxHighlighter";
import { WasmDisassemblyViewer } from "../components/WasmDisassemblyViewer";
import { UploadZone } from "../components/upload-zone";
import { CopyButton } from "../components/CopyButton";
import { ContractAddressLink } from "../components/ContractAddressLink";
import { WalletBalanceCard } from "../components/WalletBalanceCard";
import { LiquidityPoolAnalytics } from "../components/LiquidityPoolAnalytics";
import { StakingCalculator } from "../components/StakingCalculator";
import { TransactionConfetti } from "../components/TransactionConfetti";
import { TransactionHistoryTable } from "../components/TransactionHistoryTable";
import { WalletModal } from "../components/WalletModal";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { ToastStack, type ToastItem } from "../components/Toast";
import { dismissToast, enqueueToast } from "../lib/toastQueue.cjs";
import { useNetwork } from "../context/NetworkContext";
import { clearLatestAnalysis, loadLatestAnalysis, saveLatestAnalysis } from "../lib/analysisStorage";
import { ApiError, analyzeService, apiUrl } from "../lib/api";
import { createUserFriendlyMessage, extractErrorDetails, formatError } from "../lib/errorHandling";
import type { GasGolfingSuggestion } from "../lib/gasGolfingSort";
import {
  MOCK_CONTRACT_FUNCTIONS,
  generateMockResult,
  generateMockTransactions,
  type ContractFunction,
  type InvocationResult,
  type TransactionStatus,
} from "../lib/sorobantypes";
import { DEFAULT_TRANSACTION_FILTER, type TransactionFilter } from "../lib/transactionFilters";

function arrayBufferToBase64(buffer: ArrayBuffer): string {
  let binary = "";
  const bytes = new Uint8Array(buffer);
  const len = bytes.byteLength;
  for (let i = 0; i < len; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return typeof window !== "undefined"
    ? window.btoa(binary)
    : Buffer.from(binary, "binary").toString("base64");
}

// React Flow measures real DOM nodes, so the visualizer is client-only.
const SchemaVisualizer = dynamic(
  () => import("../components/SchemaVisualizer").then((mod) => mod.SchemaVisualizer),
  {
    ssr: false,
    loading: () => (
      <div className="h-[420px] animate-pulse rounded-2xl border border-slate-800 bg-slate-900/60" />
    ),
  },
);

const VALID_TABS: NavTab[] = ["explorer", "schema", "disassembly", "history", "transactions", "analytics"];

export default function Home() {
  const router = useRouter();
  const { network } = useNetwork();
  const [tab, setTab] = useState<NavTab>("explorer");
  const [contractId, setContractId] = useState(network.defaultContractId);
  const [selectedFunction, setSelectedFunction] = useState<ContractFunction>(
    MOCK_CONTRACT_FUNCTIONS[0],
  );
  const [currentResult, setCurrentResult] = useState<InvocationResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [wasmFile, setWasmFile] = useState<File | null>(null);
  const [wasmData, setWasmData] = useState<string | null>(null);
  const [toastNotifications, setToastNotifications] = useState<ToastItem[]>([]);

  const [uploadResetKey, setUploadResetKey] = useState(0);
  const mockTransactions = useMemo(() => generateMockTransactions(47), []);

  const [transactionStatusFilter, setTransactionStatusFilter] = useState<TransactionStatus | "all">(
    DEFAULT_TRANSACTION_FILTER.status,
  );
  const [transactionFunctionFilter, setTransactionFunctionFilter] = useState(
    DEFAULT_TRANSACTION_FILTER.functionName,
  );

  const transactionFilter: TransactionFilter = useMemo(
    () => ({ status: transactionStatusFilter, functionName: transactionFunctionFilter }),
    [transactionStatusFilter, transactionFunctionFilter],
  );

  const handleTransactionStatusFilterChange = useCallback((status: TransactionStatus | "all") => {
    setTransactionStatusFilter(status);
  }, []);

  const handleTransactionFunctionFilterChange = useCallback((functionName: string) => {
    setTransactionFunctionFilter(functionName);
  }, []);

  useEffect(() => {
    setContractId(network.defaultContractId);
  }, [network]);

  // Gas golfing state
  const [gasGolfingSuggestions, setGasGolfingSuggestions] = useState<GasGolfingSuggestion[]>([]);
  const [gasGolfingLoading, setGasGolfingLoading] = useState(false);
  const [gasGolfingError, setGasGolfingError] = useState<string | null>(null);

  // History hook
  const { history, addToHistory } = useInvocationHistory();

  // Restore the latest analysis result on initial page load
  useEffect(() => {
    const restored = loadLatestAnalysis();
    if (restored) {
      setCurrentResult(restored);
    }
  }, []);

  // Keep the active tab in sync with `?tab=` so the Cmd+K palette (and plain
  // links) can deep-link straight to a panel.
  useEffect(() => {
    const requested = router.query.tab;
    const value = Array.isArray(requested) ? requested[0] : requested;
    if (value && VALID_TABS.includes(value as NavTab)) {
      setTab(value as NavTab);
    }
  }, [router.query.tab]);

  // Non-navigation commands from the global search overlay.
  useEffect(() => {
    const handleCommand = (event: Event) => {
      const detail = (event as CustomEvent).detail as
        | { action?: string; payload?: { name?: string } }
        | undefined;
      if (detail?.action !== "select-function" || !detail.payload?.name) return;

      const match = MOCK_CONTRACT_FUNCTIONS.find((fn) => fn.name === detail.payload?.name);
      if (!match) return;

      setSelectedFunction(match);
      setCurrentResult(null);
      setTab("explorer");
    };

    window.addEventListener(SEARCH_COMMAND_EVENT, handleCommand);
    return () => window.removeEventListener(SEARCH_COMMAND_EVENT, handleCommand);
  }, []);

  const handleSimulate = async (inputs: Record<string, any>, customWasmData?: string) => {
    setLoading(true);
    let errorType: string | undefined;

    try {
      const activeWasmData = customWasmData ?? wasmData;
      const report = activeWasmData
        ? await analyzeService.analyzeWasm({
            wasm_bytes: activeWasmData,
            function_name: selectedFunction.name,
            args: Object.values(inputs).map((value) => String(value)),
          })
        : await analyzeService.analyze({
            contract_id: contractId,
            function_name: selectedFunction.name,
          });

      const result: InvocationResult = {
        id: Math.random().toString(36).slice(2),
        functionName: selectedFunction.name,
        inputs,
        result: generateMockResult(selectedFunction.name, inputs),
        analysisReport: report,
        resourceCost: report,
        stateSnapshot: report.state_snapshot,
        callGraphMermaid: report.call_graph_mermaid ?? undefined,
        timestamp: Date.now(),
        success: true,
      };

      setCurrentResult(result);
      addToHistory(result);
      saveLatestAnalysis(result);
      if (typeof window !== "undefined" && (window as any).triggerConfetti) {
        (window as any).triggerConfetti();
      }
    } catch (error) {
      if (error instanceof ApiError) {
        errorType = error.body?.error;
      }

      const formatted = formatError(error);

      const errorResult: InvocationResult = {
        id: Math.random().toString(36).substring(7),
        functionName: selectedFunction.name,
        inputs,
        error: formatted.message || "Analysis failed",
        errorType: errorType || "ANALYSIS_ERROR",
        timestamp: Date.now(),
        success: false,
      };
      setCurrentResult(errorResult);
      addToHistory(errorResult);
      setToastNotifications((current) =>
        enqueueToast(current, {
          id: `${Date.now()}-${Math.random()}`,
          message: formatted.message || "Simulation failed",
          type: "error",
        }),
      );
    } finally {
      setLoading(false);
    }
  };

  const handleClearAnalysis = useCallback(() => {
    setCurrentResult(null);
    setWasmData(null);
    clearLatestAnalysis();
    setUploadResetKey((k) => k + 1);
  }, []);

  const handleWasmReady = async (file: File) => {
    setGasGolfingLoading(true);
    setGasGolfingError(null);
    setGasGolfingSuggestions([]);

    try {
      const bytes = await file.arrayBuffer();
      const base64Bytes = arrayBufferToBase64(bytes);

      const res = await fetch(apiUrl("/analyze/gas-golfing"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          wasm_bytes: base64Bytes,
          contract_name: file.name.replace(/\.wasm$/i, ""),
        }),
      });

      if (!res.ok) {
        const err = await extractErrorDetails(res);
        throw new Error(createUserFriendlyMessage(err));
      }

      const data = await res.json();
      setGasGolfingSuggestions(
        (data?.report?.suggestions ?? []) as GasGolfingSuggestion[],
      );
    } catch (e) {
      setGasGolfingError(e instanceof Error ? e.message : "Failed to analyze WASM");
    } finally {
      setGasGolfingLoading(false);
    }
  };

  const analysisReport = currentResult?.analysisReport ?? currentResult?.resourceCost;

  const { pageTitle, seoDescription } = useMemo(() => {
    switch (tab) {
      case "analytics":
        return {
          pageTitle: "SoroScope | Liquidity Pool APY & TVL Analytics",
          seoDescription: "Explore historical APY, TVL, and volume charts for the XLM/USDC liquidity pool.",
        };
      case "transactions":
        return {
          pageTitle: "SoroScope | Transaction History Telemetry",
          seoDescription: "Monitor real-time Soroban contract events, transaction fees, and telemetry records.",
        };
      case "disassembly":
        return {
          pageTitle: "SoroScope | WASM Disassembly Viewer",
          seoDescription: "Decode Soroban contract WASM bytecode and cross-highlight opcodes against the Rust source.",
        };
      case "history":
        return {
          pageTitle: "SoroScope | Invocation History Analysis",
          seoDescription: "Review previous Soroban contract runs and CPU/RAM instruction summaries.",
        };
      case "explorer":
      default:
        return {
          pageTitle: `SoroScope | ${selectedFunction.name} - Contract Analyzer`,
          seoDescription: `Analyze CPU, RAM, and ledger footprint of the ${selectedFunction.name} function on contract ${contractId}.`,
        };
    }
  }, [tab, selectedFunction.name, contractId]);

  return (
    <>
      <Head>
        <title>{pageTitle}</title>
        <meta name="description" content={seoDescription} />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <link rel="icon" href="/favicon.ico" />
      </Head>
      <main className="min-h-screen bg-slate-950 text-slate-100">
        <HeaderNav tab={tab} setTab={setTab} />

        <section className="mx-auto max-w-6xl px-4 py-6 sm:px-6 lg:px-8">
          {/* WASM Upload Zone */}
          <div className="mb-6 rounded-2xl border border-slate-800 bg-slate-900/70 p-5">
            <ErrorBoundary
              fallback={(error, reset) => (
                <div className="rounded-lg border border-red-800/60 bg-red-950/30 p-6 text-center text-red-100">
                  <p className="text-sm font-semibold">Upload failed unexpectedly</p>
                  <p className="mx-auto mt-2 max-w-md text-xs leading-relaxed text-red-200/80">
                    {error.message}
                  </p>
                  <button
                    type="button"
                    onClick={reset}
                    className="mt-4 rounded-md border border-red-700/70 px-4 py-2 text-sm text-red-100 hover:bg-red-900/40"
                  >
                    Try another file
                  </button>
                </div>
              )}
            >
              <UploadZone
                key={uploadResetKey}
                backendUrl={apiUrl("/analyze/wasm")}
                enableBackendValidation={true}
                onFileReady={async (file) => {
                  setWasmFile(file);
                  const arrayBuffer = await file.arrayBuffer();
                  const base64 = arrayBufferToBase64(arrayBuffer);
                  setWasmData(base64);

                  // Trigger gas golfing analysis & simulation
                  await handleWasmReady(file);
                  await handleSimulate({}, base64);
                }}
                onReset={() => {
                  setWasmFile(null);
                  setWasmData(null);
                  setCurrentResult(null);
                  setGasGolfingSuggestions([]);
                  setGasGolfingError(null);
                }}
              />
            </ErrorBoundary>
          </div>

          {/* Side-by-side Rust/WASM panes need the full content width. */}
          {tab === "disassembly" && (
            <section className="mb-6 rounded-2xl border border-slate-800 bg-slate-900/70 p-5">
              <WasmDisassemblyViewer wasmFile={wasmFile} />
            </section>
          )}

          {/* Gas Golfing Suggestions Table */}
          {gasGolfingLoading ? (
            <div className="mb-6 rounded-lg border border-slate-800 bg-slate-900 p-4 text-sm text-slate-300">
              Analyzing WASM for Gas Golfing suggestions…
            </div>
          ) : gasGolfingError ? (
            <div className="mb-6 rounded-lg border border-amber-800 bg-amber-950/40 p-4 text-sm text-amber-200">
              {gasGolfingError}
            </div>
          ) : gasGolfingSuggestions.length > 0 ? (
            <div className="mb-6">
              <GasGolfingSuggestionsTable suggestions={gasGolfingSuggestions} />
            </div>
          ) : null}

          <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
            {/* Left Column - Function Selection & Form */}
            <div className="space-y-4">
              <WalletBalanceCard />
              <FunctionSidebar
                functions={MOCK_CONTRACT_FUNCTIONS}
                selectedFunction={selectedFunction}
                onSelect={(func) => {
                  setSelectedFunction(func);
                  setCurrentResult(null);
                }}
              />
              <div className="rounded-2xl border border-slate-800 bg-slate-900/70 p-5">
                <div className="mb-2 flex items-center justify-between">
                  <label htmlFor="contract-id-input" className="text-sm font-medium text-slate-300">
                    Contract ID
                  </label>
                  <CopyButton text={contractId} label="Copy ID" tooltipPosition="left" />
                </div>
                <input
                  id="contract-id-input"
                  value={contractId}
                  onChange={(e) => setContractId(e.target.value)}
                  placeholder="Enter Soroban contract ID"
                  aria-label="Soroban contract ID"
                  className="w-full rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 font-mono text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
                />
                <div className="mt-2 flex items-center gap-3 text-xs">
                  <ContractAddressLink
                    value={contractId}
                    kind="contract"
                    label={null}
                    className="text-xs"
                  />
                  <span className="text-slate-500">&middot;</span>
                  <span className="text-slate-400">opens on the active network explorer</span>
                </div>
                {wasmFile && (
                  <div className="mt-3 flex items-center gap-2 rounded-lg border border-emerald-500/25 bg-emerald-500/10 p-2 text-xs">
                    <span className="font-semibold text-emerald-400">Active WASM:</span>
                    <code className="font-mono text-slate-200">{wasmFile.name}</code>
                    <span className="text-slate-400">({(wasmFile.size / 1024).toFixed(1)} KB)</span>
                  </div>
                )}
              </div>
              <ContractInteraction
                selectedFunction={selectedFunction}
                loading={loading}
                onSubmit={handleSimulate}
              />
            </div>

            {/* Right Column - Results / Schema / Transactions / History / Analytics */}
            <div className="rounded-2xl border border-slate-800 bg-slate-900/70 p-5">
              {tab === "explorer" ? (
                loading ? (
                  <>
                    <ResultViewerSkeleton />
                    <div className="mt-4">
                      <NutritionLabelSkeleton />
                    </div>
                  </>
                ) : currentResult ? (
                  <>
                    <ResultViewer result={currentResult} />
                    {analysisReport && (
                      <div className="mt-4 flex flex-col gap-4">
                        <ResourceHeatmap
                          resourceCost={{
                            cpu_instructions: analysisReport.cpu_instructions,
                            ram_bytes: analysisReport.ram_bytes,
                            ledger_read_bytes: analysisReport.ledger_read_bytes,
                            ledger_write_bytes: analysisReport.ledger_write_bytes,
                            transaction_size_bytes: analysisReport.transaction_size_bytes,
                            cost_stroops: (analysisReport as any).cost_stroops,
                            state_snapshot: currentResult.stateSnapshot,
                          }}
                        />
                      </div>
                    )}
                    {analysisReport && (
                      <div className="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2">
                        <NutritionLabel
                          cpu_instructions={analysisReport.cpu_instructions}
                          ram_bytes={analysisReport.ram_bytes}
                          ledger_read_bytes={analysisReport.ledger_read_bytes}
                          ledger_write_bytes={analysisReport.ledger_write_bytes}
                          transaction_size_bytes={analysisReport.transaction_size_bytes}
                        />
                        <GasUsageChart
                          cpu_instructions={analysisReport.cpu_instructions}
                          ram_bytes={analysisReport.ram_bytes}
                          ledger_read_bytes={analysisReport.ledger_read_bytes}
                          ledger_write_bytes={analysisReport.ledger_write_bytes}
                          transaction_size_bytes={analysisReport.transaction_size_bytes}
                          cost_stroops={(analysisReport as any).cost_stroops}
                          testnetAverages={(analysisReport as any).testnet_averages}
                        />
                      </div>
                    )}
                    {analysisReport && analysisReport.cost_stroops !== undefined && (
                      <div className="mt-4">
                        <FeeEstimationPreview
                          costStroops={analysisReport.cost_stroops}
                          loading={loading}
                        />
                      </div>
                    )}
                    <button
                      type="button"
                      onClick={handleClearAnalysis}
                      className="mt-4 px-4 py-2 bg-slate-800 text-slate-200 rounded hover:bg-slate-700 transition"
                      aria-label="Clear current analysis"
                    >
                      Clear analysis
                    </button>
                  </>
                ) : (
                  <p className="text-slate-400 text-center py-8">
                    Run an analysis to see results
                  </p>
                )
              ) : tab === "schema" ? (
                <SchemaVisualizer report={analysisReport} />
              ) : tab === "disassembly" ? (
                <p className="py-8 text-center text-slate-400">Disassembly is shown above.</p>
              ) : tab === "transactions" ? (
                <TransactionHistoryTable
                  transactions={mockTransactions}
                  filter={transactionFilter}
                  onStatusFilterChange={handleTransactionStatusFilterChange}
                  onFunctionFilterChange={handleTransactionFunctionFilterChange}
                />
              ) : tab === "analytics" ? (
                <LiquidityPoolAnalytics />
              ) : (
                <InvocationHistory
                  onSelectResult={(result) => {
                    setCurrentResult(result);
                    setTab("explorer");
                  }}
                />
              )}
            </div>
          </div>

          {/* Staking & Yield Calculator Widget Section */}
          <section className="mt-8">
            <StakingCalculator />
          </section>

          {/* Contract Source & XDR Viewer Section */}
          <div className="mt-6 rounded-2xl border border-slate-800 bg-slate-900/70 p-5">
            <SourceCodeViewer currentResult={currentResult} />
          </div>
        </section>
      </main>

      <TransactionConfetti />
      <WalletModal />
      <ToastStack
        toasts={toastNotifications}
        onClose={(id) => setToastNotifications((current) => dismissToast(current, id))}
      />
    </>
  );
}

// ──────────────────────────────────────────────
// Source Code & XDR Viewer Sub-component
// ──────────────────────────────────────────────

interface SourceCodeViewerProps {
  currentResult: InvocationResult | null;
}

function SourceCodeViewer({ currentResult }: SourceCodeViewerProps) {
  const [expanded, setExpanded] = useState(false);
  const [viewMode, setViewMode] = useState<"contract" | "xdr">("contract");

  // Sample contract source code for demonstration
  const sampleContractSource = `use soroban_sdk::{contract, contractimpl, Env, Address, Symbol, symbol_short, vec, Vec};

pub trait LiquidityPool {
    fn deposit(e: Env, from: Address, amount: u128) -> bool;
    fn withdraw(e: Env, to: Address, amount: u128) -> bool;
    fn swap(e: Env, from: Address, token_in: Address, token_out: Address, amount_in: u128) -> u128;
    fn get_balance(e: Env, account: Address) -> u128;
    fn get_reserves(e: Env) -> (u128, u128);
}

#[contract]
pub struct LiquidityPoolContract;

#[contractimpl]
impl LiquidityPoolContract {
    pub fn deposit(env: Env, from: Address, amount: u128) -> bool {
        // Validate the caller
        from.require_auth();

        // Transfer tokens from user to pool
        let token = TokenClient::new(&env, &env.current_contract_address());
        token.transfer(&from, &env.current_contract_address(), &amount);

        // Mint LP tokens proportional to deposit
        let total_supply = Self::total_supply(&env);
        let lp_amount = if total_supply == 0 {
            amount
        } else {
            let reserves = Self::get_reserves(&env);
            (amount * total_supply) / reserves.0
        };

        Self::mint_lp_tokens(&env, &from, &lp_amount);
        true
    }

    pub fn get_reserves(env: Env) -> (u128, u128) {
        let reserve_a: u128 = env.storage().instance().get(&symbol_short!("res_a")).unwrap_or(0);
        let reserve_b: u128 = env.storage().instance().get(&symbol_short!("res_b")).unwrap_or(0);
        (reserve_a, reserve_b)
    }
}

fn calculate_swap_output(
    amount_in: u128,
    reserve_in: u128,
    reserve_out: u128,
) -> u128 {
    // Constant product formula: x * y = k
    let amount_in_with_fee = amount_in * 997;
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = (reserve_in * 1000) + amount_in_with_fee;
    numerator / denominator
}`;

  const sampleXdrData = `AAAAAgAAAABzdPocx0i4sJzFqNfRqI7Lq4G5GQ2xX0hYjK6Y5JXZzQAAAAoAAAAQAAAA
AAAAAQAAAAAAAAAAAAAAAFz8rXsAAAAAMgAAAAAAAAABAAAABFRSQU5TRkVSAAAAAAAAAAEA
AAAFAAAAAQAAABdUZXN0IFNvcm9iYW4gVHJhbnNmZXIAAAAAAAoAAAAEVXNkYwAAAAAAAAAA
AAAAAAAFAAAAAQAAABdUZXN0IFNvcm9iYW4gVHJhbnNmZXIAAAAAAAoAAAAFeFNvbAAAAAAA
AAAAAAFlZfTAAAAAAAAAAAIAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAFz8ra0AAAAAAAAAAQAA
AARUUkFOU0ZFUgAAAAAAAAABAAAABQAAAAEAAAAXVGVzdCBTb3JvYmFuIFRyYW5zZmVyAAAA
AAAKAAAABFVzZGMAAAAAAAAAAAAAAAAAAAAABQAAAAEAAAAXVGVzdCBTb3JvYmFuIFRyYW5z
ZmVyAAAAAAAKAAAABXhTb2wAAAAAAAAAAAAAAAABZWX0wAAAAAAAAAACAAAAAAAAAAAAAAEA
AAAAAAAAAAAAAABc/K2QAAAAAAAAAAEAAAAEVFJBTlNGRVIAAAAAAAAAAQAAAAUAAAABAAAA
F1Rlc3QgU29yb2JhbiBUcmFuc2ZlcgAAAAAACgAAAARVc2RjAAAAAAAAAAAAAAAAAAAAAAUA
AAABAAAAF1Rlc3QgU29yb2JhbiBUcmFuc2ZlcgAAAAAACgAAAAV4U29sAAAAAAAAAAAAAAAA
AWVl9MAAAAAAAAAAAgAAAAAAAAAAAAABAAAAAAAAAAAAAAAAXPytoAAAAAAAAAABAAAABFRS
QU5TRkVSAAAAAAAAAAEAAAAFAAAAAQAAABdUZXN0IFNvcm9iYW4gVHJhbnNmZXIAAAAAAAAK
AAAABFVzZGMAAAAAAAAAAAAAAAAAAAAABQAAAAEAAAAXVGVzdCBTb3JvYmFuIFRyYW5zZmVy
AAAAAAAKAAAABXhTb2wAAAAAAAAAAAAAAAABZWX0wAAAAAAAAAACAAAAAAAAAAAAAAEAAAAA
AAAAAABc/K1gAAAAAAAAAAEAAAAEVFJBTlNGRVIAAAAAAAAAAQAAAAUAAAABAAAAF1Rlc3Qg
U29yb2JhbiBUcmFuc2ZlcgAAAAAACgAAAARVc2RjAAAAAAAAAAAAAAAAAAAAAAUAAAABAAAA
F1Rlc3QgU29yb2JhbiBUcmFuc2ZlcgAAAAAACgAAAAV4U29sAAAAAAAAAAAAAAAAAWVl9MAA`;

  // Use result state snapshot or sample data
  const displayCode = currentResult?.analysisReport?.state_snapshot
    ? JSON.stringify(currentResult.analysisReport.state_snapshot.ledger_entries, null, 2)
    : viewMode === "contract"
      ? sampleContractSource
      : sampleXdrData;

  return (
    <div>
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center justify-between text-left"
        aria-expanded={expanded}
        aria-controls="source-code-panel"
        aria-label="Toggle contract source code and raw XDR transaction panel"
      >
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-slate-800">
            <Code className="h-4 w-4 text-cyan-400" aria-hidden="true" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-slate-200">
              Contract Source &amp; XDR
            </h3>
            <p className="text-xs text-slate-400">
              View contract source code and raw XDR transaction data
            </p>
          </div>
        </div>
        <span className="text-slate-400">
          {expanded ? (
            <ChevronDown className="h-5 w-5" aria-hidden="true" />
          ) : (
            <ChevronRight className="h-5 w-5" aria-hidden="true" />
          )}
        </span>
      </button>

      {expanded && (
        <div id="source-code-panel" className="mt-4 space-y-4">
          {/* View toggle */}
          <div className="flex items-center gap-2 border-b border-slate-800 pb-3" role="tablist" aria-label="Source code view options">
            <button
              type="button"
              role="tab"
              aria-selected={viewMode === "contract"}
              onClick={() => setViewMode("contract")}
              className={`flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                viewMode === "contract"
                  ? "bg-cyan-500/10 text-cyan-400 border border-cyan-500/30"
                  : "text-slate-400 hover:text-slate-300 border border-transparent"
              }`}
            >
              <FileCode className="h-3.5 w-3.5" aria-hidden="true" />
              Contract Source
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={viewMode === "xdr"}
              onClick={() => setViewMode("xdr")}
              className={`flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                viewMode === "xdr"
                  ? "bg-cyan-500/10 text-cyan-400 border border-cyan-500/30"
                  : "text-slate-400 hover:text-slate-300 border border-transparent"
              }`}
            >
              <Code className="h-3.5 w-3.5" aria-hidden="true" />
              XDR View
            </button>
          </div>

          {/* Syntax highlighted code */}
          <SyntaxHighlighter
            code={displayCode}
            language={viewMode}
            showLineNumbers
            maxHeight="480px"
          />
        </div>
      )}
    </div>
  );
}
