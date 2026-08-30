import Head from 'next/head';
import { useEffect, useState } from 'react';

import { ConnectButton } from '../components/ConnectButton';
import { ContractInteraction } from '../components/ContractInteraction';
import { ErrorBoundary } from '../components/ErrorBoundary';
import { FunctionSidebar } from '../components/FunctionSidebar';
import { GasUsageChart } from '../components/GasUsageChart';
import { InvocationHistory, useInvocationHistory } from '../components/InnovocationHistory';
import { NutritionLabel } from '../components/NutritionLabel';
import { ResultViewer } from '../components/Resultviewer';
import { ResultViewerSkeleton } from '../components/ResultViewerSkeleton';
import { NutritionLabelSkeleton } from '../components/NutritionLabelSkeleton';
import { ResourceHeatmap } from '../components/ResourceHeatmap';
import { UploadZone } from '../components/upload-zone';
import { WalletModal } from '../components/WalletModal';
import { analyzeService } from '../lib/api';
import { MOCK_CONTRACT_FUNCTIONS, generateMockResult, type ContractFunction, type InvocationResult } from '../lib/sorobantypes';

export default function Home() {
  const [contractId, setContractId] = useState('CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q');
  const [selectedFunction, setSelectedFunction] = useState<ContractFunction>(MOCK_CONTRACT_FUNCTIONS[0]);
  const [currentResult, setCurrentResult] = useState<InvocationResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [tab, setTab] = useState<'explorer' | 'history'>('explorer');
  const [wasmData, setWasmData] = useState<string | null>(null);
  const [wasmFile, setWasmFile] = useState<File | null>(null);
  const { history, addToHistory } = useInvocationHistory();

  useEffect(() => {
    setCurrentResult(null);
  }, []);

  const handleSimulate = async (inputs: Record<string, unknown>, customWasmData?: string) => {
    setLoading(true);
    const activeWasmData = customWasmData ?? wasmData;

    try {
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
        callGraphMermaid: report.call_graph_mermaid,
        timestamp: Date.now(),
        success: true,
      };

      setCurrentResult(result);
      addToHistory(result);
      return result;
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Analysis failed';
      const errorResult: InvocationResult = {
        id: Math.random().toString(36).slice(2),
        functionName: selectedFunction.name,
        inputs,
        error: message,
        errorType: 'ANALYSIS_ERROR',
        timestamp: Date.now(),
        success: false,
      };
      setCurrentResult(errorResult);
      addToHistory(errorResult);
      throw error;
    } finally {
      setLoading(false);
    }
  };

  const handleFileAnalysis = async (file: File) => {
    setWasmFile(file);
    const arrayBuffer = await file.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);
    let binary = '';
    for (const byte of bytes) {
      binary += String.fromCharCode(byte);
    }
    const base64 = window.btoa(binary);
    setWasmData(base64);
    await handleSimulate({}, base64);
  };

  const analysisReport = currentResult?.analysisReport ?? currentResult?.resourceCost;

  return (
    <>
      <Head>
        <title>SoroScope - Soroban Smart Contract Resource Analyzer</title>
        <meta name="description" content="Explore, test, and analyze the CPU, RAM, and ledger footprint of Soroban smart contracts." />
      </Head>
      <div style={{ minHeight: '100vh', backgroundColor: '#0f1117' }}>
        <header className="sticky top-0 z-[100] flex flex-col gap-4 border-b border-[#30363d] bg-[#1a1f26] px-6 py-6 sm:flex-row sm:items-center sm:justify-between sm:px-10 lg:pl-[140px] lg:pr-[125px]">
          <div className="max-w-[1200px]">
            <h1 style={{ margin: '0 0 12px 0', fontSize: '28px', fontWeight: '700', color: '#00d9ff', letterSpacing: '0.5px' }}>
              SoroScope
            </h1>
            <p style={{ margin: '0', color: '#8b949e', fontSize: '14px' }}>
              Explore and test Soroban smart contracts with precision
            </p>
          </div>
          <ConnectButton />
        </header>

        <main className="mx-auto max-w-[1200px] px-4 py-6 sm:px-6">
          <div style={{ backgroundColor: '#161b22', borderRadius: '12px', padding: '28px', marginBottom: '24px', border: '1px solid #30363d' }}>
            <ErrorBoundary fallback={(error) => <div className="rounded-lg border border-red-800/60 bg-red-950/30 p-6 text-center text-red-100">{error.message}</div>}>
              <UploadZone
                onFileReady={(file) => {
                  void handleFileAnalysis(file);
                }}
                onReset={() => {
                  setWasmFile(null);
                  setWasmData(null);
                  setCurrentResult(null);
                }}
              />
            </ErrorBoundary>
          </div>

          <div style={{ marginBottom: '24px' }}>
            {wasmFile ? (
              <div style={{ padding: '12px', borderRadius: '6px', backgroundColor: 'rgba(52, 211, 153, 0.08)', border: '1px solid rgba(52, 211, 153, 0.25)', color: '#34d399' }}>
                Active WASM: {wasmFile.name} ({(wasmFile.size / 1024).toFixed(1)} KB)
              </div>
            ) : null}
          </div>

          <div className="mb-6 grid grid-cols-1 gap-6 lg:grid-cols-2">
            <div>
              <FunctionSidebar
                functions={MOCK_CONTRACT_FUNCTIONS}
                selectedFunction={selectedFunction}
                onSelect={(func) => {
                  setSelectedFunction(func);
                  setCurrentResult(null);
                }}
              />

              <ContractInteraction
                selectedFunction={selectedFunction}
                loading={loading}
                onSubmit={(inputs) => handleSimulate(inputs)}
              />
            </div>

            <div>
              <div style={{ display: 'flex', borderBottom: '1px solid #30363d', marginBottom: '24px', backgroundColor: '#161b22', borderRadius: '8px 8px 0 0', gap: '0' }}>
                <button type="button" onClick={() => setTab('explorer')} style={{ flex: 1, padding: '12px 16px', backgroundColor: 'transparent', border: 'none', borderBottom: tab === 'explorer' ? '2px solid #00d9ff' : '2px solid transparent', cursor: 'pointer', fontSize: '14px', fontWeight: tab === 'explorer' ? '600' : '500', color: tab === 'explorer' ? '#00d9ff' : '#8b949e' }}>
                  Result
                </button>
                <button type="button" onClick={() => setTab('history')} style={{ flex: 1, padding: '12px 16px', backgroundColor: 'transparent', border: 'none', borderBottom: tab === 'history' ? '2px solid #00d9ff' : '2px solid transparent', cursor: 'pointer', fontSize: '14px', fontWeight: tab === 'history' ? '600' : '500', color: tab === 'history' ? '#00d9ff' : '#8b949e' }}>
                  History ({history.length})
                </button>
              </div>

              <div style={{ backgroundColor: '#161b22', borderRadius: '0 8px 8px 8px', padding: '24px', border: '1px solid #30363d', borderTop: 'none' }}>
                {tab === 'explorer' ? (
                  loading ? (
                    <>
                      <ResultViewerSkeleton />
                      <div className="mt-4">
                        <NutritionLabelSkeleton />
                      </div>
                    </>
                  ) : (
                    <>
                      <ResultViewer result={currentResult} />
                      {currentResult?.resourceCost && (
                        <div className="mt-4 flex flex-col gap-4">
                          <ResourceHeatmap
                            resourceCost={{
                              cpu_instructions: currentResult.resourceCost.cpu_instructions,
                              ram_bytes: currentResult.resourceCost.ram_bytes,
                              ledger_read_bytes: currentResult.resourceCost.ledger_read_bytes,
                              ledger_write_bytes: currentResult.resourceCost.ledger_write_bytes,
                              transaction_size_bytes: currentResult.resourceCost.transaction_size_bytes,
                              cost_stroops: (currentResult.resourceCost as any).cost_stroops,
                              state_snapshot: currentResult.stateSnapshot,
                            }}
                          />
                          {analysisReport && (
                            <div className="mt-4 grid grid-cols-1 lg:grid-cols-2 gap-4">
                              <NutritionLabel
                                cpu_instructions={analysisReport.cpu_instructions}
                                ram_bytes={analysisReport.ram_bytes}
                                ledger_read_bytes={analysisReport.ledger_read_bytes}
                                ledger_write_bytes={analysisReport.ledger_write_bytes}
                                transaction_size_bytes={analysisReport.transaction_size_bytes}
                              />
                              <GasUsageChart
                                cpu_instructions={currentResult.resourceCost.cpu_instructions}
                                ram_bytes={currentResult.resourceCost.ram_bytes}
                                ledger_read_bytes={currentResult.resourceCost.ledger_read_bytes}
                                ledger_write_bytes={currentResult.resourceCost.ledger_write_bytes}
                                transaction_size_bytes={currentResult.resourceCost.transaction_size_bytes}
                                cost_stroops={currentResult.resourceCost.cost_stroops}
                                testnetAverages={currentResult.resourceCost.testnet_averages}
                              />
                            </div>
                          )}
                        </div>
                      )}
                    </>
                  )
                ) : (
                  <InvocationHistory onSelectResult={(result) => {
                    setCurrentResult(result);
                    setTab('explorer');
                  }} />
                )}
              </div>
            </div>
          </div>
        </main>
        <WalletModal />
      </div>
    </>
  );
}
