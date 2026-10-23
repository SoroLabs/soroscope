import Head from "next/head";
import dynamic from "next/dynamic";
import { useRouter } from "next/router";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Code, FileCode, ChevronDown, ChevronRight } from "lucide-react";

import { HeaderNav, type NavTab } from "../components/HeaderNav";
import { SEARCH_COMMAND_EVENT } from "../components/GlobalSearchModal";
import { ContractAddressLink } from "../components/ContractAddressLink";
import { ContractInteraction } from "../components/ContractInteraction";
import { CopyButton } from "../components/CopyButton";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { FeeEstimationPreview } from "../components/FeeEstimationPreview";
import { FunctionSidebar } from "../components/FunctionSidebar";
import { GasGolfingSuggestionsTable } from "../components/GasGolfingSuggestionsTable";
import { GasUsageChart } from "../components/GasUsageChart";
import { InvocationHistory, useInvocationHistory } from "../components/InnovocationHistory";
import { NutritionLabel } from "../components/NutritionLabel";
import { NutritionLabelSkeleton } from "../components/NutritionLabelSkeleton";
import { ResourceHeatmap } from "../components/ResourceHeatmap";
import { ResultViewer } from "../components/Resultviewer";
import ResultViewerSkeleton from "../components/ResultViewerSkeleton";
import { StakingCalculator } from "../components/StakingCalculator";
import { SyntaxHighlighter } from "../components/SyntaxHighlighter";
import { TransactionConfetti } from "../components/TransactionConfetti";
import { TransactionHistoryTable } from "../components/TransactionHistoryTable";
import { UploadZone } from "../components/upload-zone";
import { WalletBalanceCard } from "../components/WalletBalanceCard";
import { WalletModal } from "../components/WalletModal";
import { WasmDisassemblyViewer } from "../components/WasmDisassemblyViewer";
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
  type SimulationInputs,
  type TransactionStatus,
} from "../lib/sorobantypes";
import { emitToast } from "../lib/toastEvents";
import { DEFAULT_TRANSACTION_FILTER, type TransactionFilter } from "../lib/transactionFilters";

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

const VALID_TABS: NavTab[] = ["explorer", "schema", "disassembly", "history", "transactions"];

function arrayBufferToBase64(buffer: ArrayBuffer): string {
  let binary = "";
  const bytes = new Uint8Array(buffer);
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return window.btoa(binary);
}

/** Overrides for a single simulation run. Unset fields use current page state. */
interface SimulationOverrides {
  wasmData?: string | null;
  contractId?: string;
}

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
  const [uploadResetKey, setUploadResetKey] = useState(0);
  const mockTransactions = useMemo(() => generateMockTransactions(47), []);

  // Gas golfing state
  const [gasGolfingSuggestions, setGasGolfingSuggestions] = useState<GasGolfingSuggestion[]>([]);
  const [gasGolfingLoading, setGasGolfingLoading] = useState(false);
  const [gasGolfingError, setGasGolfingError] = useState<string | null>(null);

  const { addToHistory } = useInvocationHistory();

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

  // Restore the latest analysis result on initial page load.
  useEffect(() => {
    let cancelled = false;
    void loadLatestAnalysis().then((restored) => {
      if (!cancelled && restored) setCurrentResult(restored);
    });
    return () => {
      cancelled = true;
    };
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

  /** Inputs of the last simulation, so it can be re-run on another network. */
  const lastRunInputsRef = useRef<SimulationInputs | null>(null);

  const handleSimulate = async (inputs: SimulationInputs, overrides: SimulationOverrides = {}) => {
    setLoading(true);
    lastRunInputsRef.current = inputs;
    let errorType: string | undefined;

    try {
      const activeWasmData = overrides.wasmData !== undefined ? overrides.wasmData : wasmData;
      const activeContractId = overrides.contractId ?? contractId;
      const report = activeWasmData
        ? await analyzeService.analyzeWasm({
            wasm_bytes: activeWasmData,
            function_name: selectedFunction.name,
            args: Object.values(inputs).map((value) => String(value)),
          })
        : await analyzeService.analyze({
            contract_id: activeContractId,
            function_name: selectedFunction.name,
            args: Object.values(inputs).map((value) => String(value)),
          });

      const result: InvocationResult = {
        id: Math.random().toString(36).slice(2),
        functionName: selectedFunction.name,
        inputs,
        result: generateMockResult(selectedFunction.name, inputs),
        analysisReport: report,
        resourceCost: report,
        stateSnapshot: report.state_snapshot ?? undefined,
        callGraph: report.call_graph ?? undefined,
        callGraphMermaid: report.call_graph_mermaid ?? undefined,
        timestamp: Date.now(),
        success: true,
      };

      setCurrentResult(result);
      addToHistory(result);
      void saveLatestAnalysis(result);
      const confetti = (window as { triggerConfetti?: () => void }).triggerConfetti;
      confetti?.();
    } catch (error) {
      if (error instanceof ApiError) {
        const body = error.body as { error?: unknown } | null;
        if (typeof body?.error === "string") errorType = body.error;
      }

      const formatted = formatError(error);
      const message = formatted.message || "Analysis failed";

      const errorResult: InvocationResult = {
        id: Math.random().toString(36).slice(2),
        functionName: selectedFunction.name,
        inputs,
        error: message,
        errorType: errorType || "ANALYSIS_ERROR",
        timestamp: Date.now(),
        success: false,
      };
      setCurrentResult(errorResult);
      addToHistory(errorResult);
      emitToast({ message, type: "error" });
    } finally {
      setLoading(false);
    }
  };

  // Always call the latest `handleSimulate` from effects below.
  const handleSimulateRef = useRef(handleSimulate);
  handleSimulateRef.current = handleSimulate;

  // Switching network loads that network's default contract and re-runs the
  // last simulation (if any) against it.
  const previousNetworkIdRef = useRef(network.id);
  useEffect(() => {
    if (previousNetworkIdRef.current === network.id) return;
    previousNetworkIdRef.current = network.id;

    setContractId(network.defaultContractId);
    const lastInputs = lastRunInputsRef.current;
    if (lastInputs) {
      void handleSimulateRef.current(lastInputs, { contractId: network.defaultContractId });
    }
  }, [network]);

  const runGasGolfing = async (file: File, base64Bytes: string) => {
    setGasGolfingLoading(true);
    setGasGolfingError(null);
    setGasGolfingSuggestions([]);

    try {
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
      setGasGolfingSuggestions((data?.report?.suggestions ?? []) as GasGolfingSuggestion[]);
    } catch (e) {
      setGasGolfingError(e instanceof Error ? e.message : "Failed to analyze WASM");
    } finally {
      setGasGolfingLoading(false);
    }
  };

  const handleWasmReady = async (file: File) => {
    setWasmFile(file);
    const base64 = arrayBufferToBase64(await file.arrayBuffer());
    setWasmData(base64);
    await Promise.all([runGasGolfing(file, base64), handleSimulate({}, { wasmData: base64 })]);
  };

  const handleWasmReset = () => {
    setWasmFile(null);
    setWasmData(null);
    setCurrentResult(null);
    setGasGolfingSuggestions([]);
    setGasGolfingError(null);
  };

  const handleClearAnalysis = useCallback(() => {
    setCurrentResult(null);
    setWasmFile(null);
    setWasmData(null);
    setGasGolfingSuggestions([]);
    setGasGolfingError(null);
    lastRunInputsRef.current = null;
    clearLatestAnalysis();
    setUploadResetKey((k) => k + 1);
  }, []);

  const analysisReport = currentResult?.analysisReport;
  const legacyCost = currentResult?.resourceCost;
  const resources = analysisReport ?? legacyCost;

  const { pageTitle, seoDescription } = useMemo(() => {
    switch (tab) {
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
      case "schema":
        return {
          pageTitle: "SoroScope | Contract Schema Visualizer",
          seoDescription: "Visualize the cross-contract call tree and storage keys touched by a Soroban invocation.",
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
            <div className="mb-4">
              <h2 className="text-base font-semibold text-slate-200">Upload Contract</h2>
              <p className="text-sm text-slate-500">
                Drop a compiled Soroban contract (.wasm) to analyze its resource usage
              </p>
            </div>
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
                onFileReady={(file) => void handleWasmReady(file)}
                onReset={handleWasmReset}
              />
            </ErrorBoundary>
          </div>

          {/* Side-by-side Rust/WASM panes need the full content width. */}
          {tab === "disassembly" && (
            <section className="mb-6 rounded-2xl border border-slate-800 bg-slate-900/70 p-5">
              <WasmDisassemblyViewer wasmFile={wasmFile} />
            </section>
          )}

          {/* Gas Golfing Suggestions */}
          {(gasGolfingLoading || gasGolfingError || gasGolfingSuggestions.length > 0) && (
            <div className="mb-6">
              {gasGolfingLoading ? (
                <div className="rounded-lg border border-slate-800 bg-slate-900/70 p-4 text-sm text-slate-400">
                  Analyzing WASM for Gas Golfing suggestions…
                </div>
              ) : gasGolfingError ? (
                <div className="rounded-lg border border-orange-500/60 bg-slate-900/70 p-4 text-sm text-orange-300">
                  {gasGolfingError}
                </div>
              ) : (
                <GasGolfingSuggestionsTable suggestions={gasGolfingSuggestions} />
              )}
            </div>
          )}

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
                  className="w-full rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 font-mono text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
                />
                <div className="mt-2 flex items-center gap-3 text-xs">
                  <ContractAddressLink value={contractId} kind="contract" label={null} className="text-xs" />
                  <span className="text-slate-600">&middot;</span>
                  <span className="text-slate-500">opens on the active network explorer</span>
                </div>
                {wasmFile && (
                  <div className="mt-4 flex items-center gap-2 rounded-md border border-emerald-400/25 bg-emerald-400/10 p-3">
                    <span className="text-xs font-semibold text-emerald-400">Active WASM:</span>
                    <code className="font-mono text-xs text-slate-300">{wasmFile.name}</code>
                    <span className="text-[11px] text-slate-500">({(wasmFile.size / 1024).toFixed(1)} KB)</span>
                  </div>
                )}
              </div>
              <ContractInteraction
                selectedFunction={selectedFunction}
                loading={loading}
                onSubmit={(inputs) => handleSimulate(inputs)}
              />
            </div>

            {/* Right Column - Results */}
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
                    <ResultViewer result={currentResult} contractId={contractId} />
                    {resources && (
                      <div className="mt-4 flex flex-col gap-4">
                        <ResourceHeatmap
                          resourceCost={{
                            cpu_instructions: resources.cpu_instructions,
                            ram_bytes: resources.ram_bytes,
                            ledger_read_bytes: resources.ledger_read_bytes,
                            ledger_write_bytes: resources.ledger_write_bytes,
                            transaction_size_bytes: resources.transaction_size_bytes,
                            cost_stroops: resources.cost_stroops,
                            state_snapshot: currentResult.stateSnapshot,
                          }}
                        />
                        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                          <NutritionLabel
                            cpu_instructions={resources.cpu_instructions}
                            ram_bytes={resources.ram_bytes}
                            ledger_read_bytes={resources.ledger_read_bytes}
                            ledger_write_bytes={resources.ledger_write_bytes}
                            transaction_size_bytes={resources.transaction_size_bytes}
                            cost_stroops={resources.cost_stroops}
                            functionName={currentResult.functionName}
                          />
                          <GasUsageChart
                            cpu_instructions={resources.cpu_instructions}
                            ram_bytes={resources.ram_bytes}
                            ledger_read_bytes={resources.ledger_read_bytes}
                            ledger_write_bytes={resources.ledger_write_bytes}
                            transaction_size_bytes={resources.transaction_size_bytes}
                            cost_stroops={resources.cost_stroops}
                            testnetAverages={resources.testnet_averages}
                          />
                        </div>
                        {resources.cost_stroops !== undefined && (
                          <FeeEstimationPreview costStroops={resources.cost_stroops} loading={loading} />
                        )}
                      </div>
                    )}
                    <button
                      type="button"
                      onClick={handleClearAnalysis}
                      className="mt-4 rounded bg-slate-800 px-4 py-2 text-slate-300 transition hover:bg-slate-700"
                    >
                      Clear analysis
                    </button>
                  </>
                ) : (
                  <p className="py-8 text-center text-slate-500">Run an analysis to see results</p>
                )
              ) : tab === "schema" ? (
                <SchemaVisualizer report={analysisReport} />
              ) : tab === "disassembly" ? (
                <p className="py-8 text-center text-slate-500">Disassembly is shown above.</p>
              ) : tab === "transactions" ? (
                <TransactionHistoryTable
                  transactions={mockTransactions}
                  filter={transactionFilter}
                  onStatusFilterChange={handleTransactionStatusFilterChange}
                  onFunctionFilterChange={handleTransactionFunctionFilterChange}
                />
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

          {/* Contract Source & XDR Viewer Section */}
          <div className="mt-6 rounded-2xl border border-slate-800 bg-slate-900/70 p-5">
            <SourceCodeViewer currentResult={currentResult} />
          </div>

          {/* Staking & Yield Calculator Widget Section */}
          <section className="mt-8">
            <StakingCalculator />
          </section>
        </section>
      </main>

      <TransactionConfetti />
      <WalletModal />
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
      >
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-slate-800">
            <Code className="h-4 w-4 text-cyan-400" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-slate-200">
              Contract Source &amp; XDR
            </h3>
            <p className="text-xs text-slate-500">
              View contract source code and raw XDR transaction data
            </p>
          </div>
        </div>
        <span className="text-slate-500">
          {expanded ? (
            <ChevronDown className="h-5 w-5" />
          ) : (
            <ChevronRight className="h-5 w-5" />
          )}
        </span>
      </button>

      {expanded && (
        <div id="source-code-panel" className="mt-4 space-y-4">
          {/* View toggle */}
          <div className="flex items-center gap-2 border-b border-slate-800 pb-3">
            <button
              type="button"
              onClick={() => setViewMode("contract")}
              className={`flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                viewMode === "contract"
                  ? "bg-cyan-500/10 text-cyan-400 border border-cyan-500/30"
                  : "text-slate-400 hover:text-slate-300 border border-transparent"
              }`}
            >
              <FileCode className="h-3.5 w-3.5" />
              Contract Source
            </button>
            <button
              type="button"
              onClick={() => setViewMode("xdr")}
              className={`flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                viewMode === "xdr"
                  ? "bg-cyan-500/10 text-cyan-400 border border-cyan-500/30"
                  : "text-slate-400 hover:text-slate-300 border border-transparent"
              }`}
            >
              <Code className="h-3.5 w-3.5" />
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
