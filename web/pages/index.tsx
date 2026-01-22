import Head from 'next/head';
import { WasmUploadZone } from '../components/upload-zone'; 

export default function Home() {
  return (
    <>
      <Head>
        <title>SoroScope Dashboard</title>
        <meta name="description" content="Soroban resource profiler dashboard" />
      </Head>
      
      <main className="min-h-screen bg-slate-950 text-slate-100 flex flex-col items-center justify-center p-4">
        
        <div className="text-center">
          <h1 className="text-4xl font-bold mb-4">SoroScope</h1>
          <p className="text-slate-300">Soroban Resource Profiler – Web Dashboard</p>
        </div>

        <div className="w-full max-w-2xl mt-10">
          <WasmUploadZone />
        </div>

      </main>
    </>
  );
}