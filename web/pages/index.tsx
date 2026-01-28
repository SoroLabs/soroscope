import Head from "next/head";
import { ConnectButton } from "../components/ConnectButton";
import { WalletModal } from "../components/WalletModal";

export default function Home() {
  return (
    <>
      <Head>
        <title>SoroScope Dashboard</title>
        <meta
          name="description"
          content="Soroban Resource Profiler – Web Dashboard"
        />
      </Head>

      {/* Header */}
      <header className="fixed top-0 left-0 right-0 z-30 bg-slate-950/80 backdrop-blur-sm border-b border-slate-800">
        <div className="max-w-7xl mx-auto px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h1 className="text-2xl font-bold text-white">SoroScope</h1>
          </div>

          {/* Wallet Connection in Top-Right */}
          <div className="flex items-center gap-4">
            <ConnectButton />
          </div>
        </div>
      </header>

      <main className="min-h-screen bg-slate-950 text-slate-100 pt-20">
        <div className="max-w-7xl mx-auto px-6 py-8">
          <div className="text-center mb-12">
            <h1 className="text-4xl font-bold mb-4">SoroScope</h1>
            <p className="text-slate-300">
              Soroban Resource Profiler – Web Dashboard
            </p>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <h3 className="text-xl font-semibold mb-3">Resource Profiling</h3>
              <p className="text-slate-400">
                Analyze and profile Soroban smart contract resource usage
              </p>
            </div>

            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <h3 className="text-xl font-semibold mb-3">
                Performance Metrics
              </h3>
              <p className="text-slate-400">
                Track CPU, memory, and storage consumption in real-time
              </p>
            </div>

            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <h3 className="text-xl font-semibold mb-3">Optimize Contracts</h3>
              <p className="text-slate-400">
                Identify bottlenecks and optimize your smart contracts
              </p>
            </div>
          </div>
        </div>
      </main>

      {/* Wallet Modal */}
      <WalletModal />
    </>
  );
}
