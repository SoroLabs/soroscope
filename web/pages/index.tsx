
import { WalletModal } from "../components/WalletModal";
import { ConnectButton } from "../components/ConnectButton";
import { useState, useEffect } from 'react';
import { DynamicForm } from '../components/DynamicForm';
import { ResultViewer } from '../components/Resultviewer';
import { InvocationHistory, useInvocationHistory } from '../components/InnovocationHistory';
import { MOCK_CONTRACT_FUNCTIONS, generateMockResult, generateMockResourceCost } from '../lib/sorobantypes';
import type { ContractFunction, InvocationResult } from '../lib/sorobantypes';

export default function Home() {
  const [contractId, setContractId] = useState('CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q');
  const [selectedFunction, setSelectedFunction] = useState<ContractFunction>(MOCK_CONTRACT_FUNCTIONS[0]);
  const [currentResult, setCurrentResult] = useState<InvocationResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [tab, setTab] = useState<'explorer' | 'history'>('explorer');
  const { history, addToHistory } = useInvocationHistory();

  const handleSimulate = async (inputs: Record<string, any>) => {
    setLoading(true);
    try {
      // Simulate API delay
      await new Promise((resolve) => setTimeout(resolve, 800));

      const result: InvocationResult = {
        id: Math.random().toString(36).substring(7),
        functionName: selectedFunction.name,
        inputs,
        result: generateMockResult(selectedFunction.name, inputs),
        resourceCost: generateMockResourceCost(),
        timestamp: Date.now(),
        success: true,
      };

      setCurrentResult(result);
      addToHistory(result);
    } catch (error) {
      const errorResult: InvocationResult = {
        id: Math.random().toString(36).substring(7),
        functionName: selectedFunction.name,
        inputs,
        error: error instanceof Error ? error.message : 'Unknown error',
        timestamp: Date.now(),
        success: false,
      };
      setCurrentResult(errorResult);
      addToHistory(errorResult);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ minHeight: '100vh', backgroundColor: '#0f1117' }}>
      {/* Header */}
      <header
        style={{
          backgroundColor: '#1a1f26',
          borderBottom: '1px solid #30363d',
          padding: '24px 0',
          position: 'sticky',
          top: 0,
          zIndex: 100,
          display:'flex',
          justifyContent:'space-between'
        }}
      >
        <div style={{ maxWidth: '1200px', paddingLeft:'140px'}}>
          <h1 style={{ margin: '0 0 12px 0', fontSize: '28px', fontWeight: '700', color: '#00d9ff', letterSpacing: '0.5px' }}>
            SoroScope
          </h1>
          <p style={{ margin: '0', color: '#8b949e', fontSize: '14px' }}>
            Explore and test Soroban smart contracts with precision
          </p>
        </div>

         {/* Wallet Connection in Top-Right */}
          <div className="pr-[125px]">
            <ConnectButton />
         </div>
      </header>

      {/* Main Content */}
      <main style={{ maxWidth: '1200px', margin: '0 auto', padding: '24px' }}>
        {/* Contract ID Input */}
        <div
          style={{
            backgroundColor: '#161b22',
            borderRadius: '8px',
            padding: '24px',
            marginBottom: '24px',
            border: '1px solid #30363d',
          }}
        >
          <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500', color: '#c9d1d9' }}>
            Contract ID
          </label>
          <input
            type="text"
            value={contractId}
            onChange={(e) => setContractId(e.target.value)}
            placeholder="Enter Soroban contract ID"
            style={{
              width: '100%',
              padding: '12px 16px',
              border: '1px solid #30363d',
              borderRadius: '6px',
              fontSize: '14px',
              fontFamily: 'monospace',
              boxSizing: 'border-box',
              backgroundColor: '#0d1117',
              color: '#c9d1d9',
            }}
          />
          <p style={{ margin: '8px 0 0 0', fontSize: '12px', color: '#8b949e' }}>
            Contract ID: <code style={{ color: '#00d9ff' }}>{contractId.substring(0, 20)}...</code>
          </p>
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '24px', marginBottom: '24px' }}>
          {/* Left Column - Function Selection & Form */}
          <div>
            <div
              style={{
                backgroundColor: '#161b22',
                borderRadius: '8px',
                padding: '24px',
                marginBottom: '24px',
                border: '1px solid #30363d',
              }}
            >
              <h2
                style={{
                  margin: '0 0 16px 0',
                  fontSize: '18px',
                  fontWeight: '600',
                  color: '#58a6ff',
                }}
              >
                Available Functions
              </h2>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                {MOCK_CONTRACT_FUNCTIONS.map((func) => (
                  <button
                    key={func.name}
                    onClick={() => {
                      setSelectedFunction(func);
                      setCurrentResult(null);
                    }}
                    style={{
                      padding: '12px 16px',
                      backgroundColor:
                        selectedFunction.name === func.name ? '#8957e5' : '#21262d',
                      color: selectedFunction.name === func.name ? '#fff' : '#c9d1d9',
                      border: selectedFunction.name === func.name ? '1px solid #8957e5' : '1px solid #30363d',
                      borderRadius: '6px',
                      textAlign: 'left',
                      cursor: 'pointer',
                      fontWeight: selectedFunction.name === func.name ? '600' : '500',
                      transition: 'all 0.2s',
                      fontSize: '14px',
                    }}
                    onMouseEnter={(e) => {
                      if (selectedFunction.name !== func.name) {
                        e.currentTarget.style.backgroundColor = '#21262d';
                        e.currentTarget.style.borderColor = '#8957e5';
                      }
                    }}
                    onMouseLeave={(e) => {
                      if (selectedFunction.name !== func.name) {
                        e.currentTarget.style.backgroundColor = '#21262d';
                        e.currentTarget.style.borderColor = '#30363d';
                      }
                    }}
                  >
                    <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                      <span>{func.name}</span>
                      <span style={{ fontSize: '12px', opacity: '0.7' }}>
                        {func.inputs.length} args
                      </span>
                    </div>
                  </button>
                ))}
              </div>
            </div>

            {/* Form */}
            <div
              style={{
                backgroundColor: '#161b22',
                borderRadius: '8px',
                padding: '24px',
                border: '1px solid #30363d',
              }}
            >
              <h2
                style={{
                  margin: '0 0 16px 0',
                  fontSize: '18px',
                  fontWeight: '600',
                  color: '#58a6ff',
                }}
              >
                {selectedFunction.name}
              </h2>
              <DynamicForm
                func={selectedFunction}
                onSubmit={handleSimulate}
                loading={loading}
              />
            </div>
          </div>

          {/* Right Column - Results & History Tabs */}
          <div>
            {/* Tabs */}
            <div
              style={{
                display: 'flex',
                borderBottom: '1px solid #30363d',
                marginBottom: '24px',
                backgroundColor: '#161b22',
                borderRadius: '8px 8px 0 0',
                gap: '0',
              }}
            >
              <button
                onClick={() => setTab('explorer')}
                style={{
                  flex: 1,
                  padding: '12px 16px',
                  backgroundColor: 'transparent',
                  border: 'none',
                  borderBottom: tab === 'explorer' ? '2px solid #00d9ff' : 'none',
                  cursor: 'pointer',
                  fontSize: '14px',
                  fontWeight: tab === 'explorer' ? '600' : '500',
                  color: tab === 'explorer' ? '#00d9ff' : '#8b949e',
                }}
              >
                Result
              </button>
              <button
                onClick={() => setTab('history')}
                style={{
                  flex: 1,
                  padding: '12px 16px',
                  backgroundColor: 'transparent',
                  border: 'none',
                  borderBottom: tab === 'history' ? '2px solid #00d9ff' : 'none',
                  cursor: 'pointer',
                  fontSize: '14px',
                  fontWeight: tab === 'history' ? '600' : '500',
                  color: tab === 'history' ? '#00d9ff' : '#8b949e',
                }}
              >
                History ({history.length})
              </button>
            </div>

            {/* Tab Content */}
            <div
              style={{
                backgroundColor: '#161b22',
                borderRadius: '0 8px 8px 8px',
                padding: '24px',
                border: '1px solid #30363d',
                borderTop: 'none',
              }}
            >
              {tab === 'explorer' ? (
                <ResultViewer result={currentResult} />
              ) : (
                <InvocationHistory onSelectResult={(result) => {
                  setCurrentResult(result);
                  setTab('explorer');
                }} />
              )}
            </div>
          </div>
        </div>

        {/* Info Cards */}
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
            gap: '16px',
          }}
        >
          <div
            style={{
              backgroundColor: '#161b22',
              borderRadius: '8px',
              padding: '16px',
              borderLeft: '4px solid #00d9ff',
              border: '1px solid #30363d',
            }}
          >
            <h3
              style={{
                margin: '0 0 8px 0',
                fontSize: '14px',
                fontWeight: '600',
                color: '#00d9ff',
              }}
            >
              Simulate
            </h3>
            <p style={{ margin: '0', fontSize: '13px', color: '#8b949e' }}>
              Preview contract execution without signing or spending XLM
            </p>
          </div>

          <div
            style={{
              backgroundColor: '#161b22',
              borderRadius: '8px',
              padding: '16px',
              borderLeft: '4px solid #a371f7',
              border: '1px solid #30363d',
            }}
          >
            <h3
              style={{
                margin: '0 0 8px 0',
                fontSize: '14px',
                fontWeight: '600',
                color: '#a371f7',
              }}
            >
              Invoke
            </h3>
            <p style={{ margin: '0', fontSize: '13px', color: '#8b949e' }}>
              Execute real transactions via your connected wallet (Freighter/xBull)
            </p>
          </div>

          <div
            style={{
              backgroundColor: '#161b22',
              borderRadius: '8px',
              padding: '16px',
              borderLeft: '4px solid #fb8500',
              border: '1px solid #30363d',
            }}
          >
            <h3
              style={{
                margin: '0 0 8px 0',
                fontSize: '14px',
                fontWeight: '600',
                color: '#fb8500',
              }}
            >
              History
            </h3>
            <p style={{ margin: '0', fontSize: '13px', color: '#8b949e' }}>
              Track all function calls with full details and resource costs
            </p>
          </div>
        </div>
      </main>
       {/* Wallet Modal */}
       <WalletModal />
    </div>
  );
}

