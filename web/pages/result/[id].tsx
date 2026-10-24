import Head from 'next/head';
import Link from 'next/link';
import { useRouter } from 'next/router';
import { useEffect, useState } from 'react';

import { ResultViewer } from '../../components/Resultviewer';
import ResultViewerSkeleton from '../../components/ResultViewerSkeleton';
import { ApiError, simulationService } from '../../lib/api';
import { isValidSimulationId, simulationRecordToResult } from '../../lib/shareLinks';
import type { InvocationResult } from '../../lib/sorobantypes';

type LoadState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready'; result: InvocationResult; contractId?: string };

/**
 * Shared simulation result page (#1286). Opening
 * `/result/<simulation_id>` fetches the persisted simulation from the
 * server and renders it exactly like the original result.
 */
export default function SharedResultPage() {
  const router = useRouter();
  const rawId = router.query.id;
  const id = Array.isArray(rawId) ? rawId[0] : rawId;
  const [state, setState] = useState<LoadState>({ status: 'loading' });

  useEffect(() => {
    if (!router.isReady) return;

    if (!id || !isValidSimulationId(id)) {
      setState({ status: 'error', message: 'This share link is not valid.' });
      return;
    }

    let cancelled = false;
    setState({ status: 'loading' });

    simulationService
      .get(id)
      .then((record) => {
        if (cancelled) return;
        const { result, contractId } = simulationRecordToResult(record, id);
        setState({ status: 'ready', result, contractId });
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        const message =
          error instanceof ApiError && error.status === 404
            ? 'This simulation result was not found. It may have expired or the link may be wrong.'
            : error instanceof Error
              ? error.message
              : 'Failed to load the simulation result.';
        setState({ status: 'error', message });
      });

    return () => {
      cancelled = true;
    };
  }, [router.isReady, id]);

  const title =
    state.status === 'ready'
      ? `SoroScope | Shared result: ${state.result.functionName}`
      : 'SoroScope | Shared simulation result';

  return (
    <>
      <Head>
        <title>{title}</title>
        <meta name="description" content="A shared Soroban contract simulation result from SoroScope." />
        <meta name="robots" content="noindex" />
      </Head>

      <main className="min-h-screen bg-slate-950 px-4 py-8 text-slate-100 sm:px-6">
        <div className="mx-auto max-w-[960px]">
          <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
            <div>
              <h1 className="text-2xl font-bold text-[#00d9ff]">Shared simulation result</h1>
              {state.status === 'ready' && state.contractId ? (
                <p className="mt-1 break-all font-mono text-xs text-slate-400">
                  {state.contractId} · {state.result.functionName}
                </p>
              ) : null}
            </div>
            <Link
              href="/"
              className="rounded-md border border-slate-700 px-3 py-1.5 text-sm text-slate-300 hover:bg-slate-800"
            >
              Open SoroScope
            </Link>
          </div>

          {state.status === 'loading' ? (
            <ResultViewerSkeleton />
          ) : state.status === 'error' ? (
            <div
              role="alert"
              className="rounded-lg border border-[#fb8500] bg-[#0d1117] p-6 text-sm text-[#f0883e]"
            >
              {state.message}
            </div>
          ) : (
            <ResultViewer result={state.result} contractId={state.contractId} />
          )}
        </div>
      </main>
    </>
  );
}
