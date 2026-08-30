import Head from 'next/head';
import Link from 'next/link';
import { useCallback, useEffect, useState } from 'react';
import { ArrowLeft, CheckCircle2, Loader2, RotateCcw, Save, XCircle } from 'lucide-react';

import { useNetwork } from '../context/NetworkContext';
import { API_URL } from '../lib/api';
import {
  DEFAULT_SETTINGS,
  clearSettings,
  loadSettings,
  saveSettings,
  validateEndpointUrl,
  validateSettings,
} from '../lib/userSettings';
import type { UserSettings } from '../lib/userSettings';

type TestState = { status: 'idle' | 'testing' | 'ok' | 'fail'; message?: string; latencyMs?: number };

const IDLE: TestState = { status: 'idle' };

/** Probe an endpoint and report whether it answered. */
async function probeEndpoint(url: string, body: object | null, timeoutMs: number): Promise<TestState> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const started = Date.now();

  try {
    const response = await fetch(url, {
      method: body ? 'POST' : 'GET',
      headers: body ? { 'Content-Type': 'application/json' } : undefined,
      body: body ? JSON.stringify(body) : undefined,
      signal: controller.signal,
    });

    const latencyMs = Date.now() - started;

    // Any HTTP answer proves the host is reachable and speaking HTTP; a 404 on
    // a health path still means the endpoint itself is live.
    return response.ok || response.status === 404
      ? { status: 'ok', message: `Reachable (HTTP ${response.status})`, latencyMs }
      : { status: 'fail', message: `Endpoint responded with HTTP ${response.status}`, latencyMs };
  } catch (error) {
    const message =
      error instanceof DOMException && error.name === 'AbortError'
        ? `No response within ${timeoutMs}ms`
        : 'Could not reach the endpoint (network error or CORS blocked)';
    return { status: 'fail', message };
  } finally {
    clearTimeout(timer);
  }
}

export default function SettingsPage() {
  const { network } = useNetwork();
  const [settings, setSettings] = useState<UserSettings>(DEFAULT_SETTINGS);
  const [errors, setErrors] = useState<Partial<Record<keyof UserSettings, string>>>({});
  const [saved, setSaved] = useState(false);
  const [rpcTest, setRpcTest] = useState<TestState>(IDLE);
  const [indexerTest, setIndexerTest] = useState<TestState>(IDLE);

  // LocalStorage is only readable after mount (SSR has no window).
  useEffect(() => {
    setSettings(loadSettings());
  }, []);

  const update = useCallback((key: keyof UserSettings, value: string | number) => {
    setSettings((prev) => ({ ...prev, [key]: value }));
    setSaved(false);
    setErrors((prev) => ({ ...prev, [key]: undefined }));
    if (key === 'rpcUrl') setRpcTest(IDLE);
    if (key === 'indexerUrl') setIndexerTest(IDLE);
  }, []);

  const handleSave = useCallback(() => {
    const result = validateSettings(settings);
    if (!result.valid) {
      setErrors(result.errors);
      setSaved(false);
      return;
    }

    setSettings(saveSettings(settings));
    setErrors({});
    setSaved(true);
  }, [settings]);

  const handleReset = useCallback(() => {
    setSettings(clearSettings());
    setErrors({});
    setSaved(false);
    setRpcTest(IDLE);
    setIndexerTest(IDLE);
  }, []);

  const testRpc = useCallback(async () => {
    const target = validateEndpointUrl(settings.rpcUrl);
    if (!target.valid || target.normalized === '') {
      setRpcTest({ status: 'fail', message: target.error ?? 'Enter an RPC URL to test' });
      return;
    }

    setRpcTest({ status: 'testing' });
    // Soroban RPC speaks JSON-RPC; getHealth is the cheapest liveness probe.
    setRpcTest(
      await probeEndpoint(
        target.normalized,
        { jsonrpc: '2.0', id: 1, method: 'getHealth' },
        settings.requestTimeoutMs,
      ),
    );
  }, [settings.rpcUrl, settings.requestTimeoutMs]);

  const testIndexer = useCallback(async () => {
    const target = validateEndpointUrl(settings.indexerUrl);
    if (!target.valid || target.normalized === '') {
      setIndexerTest({ status: 'fail', message: target.error ?? 'Enter an indexer URL to test' });
      return;
    }

    setIndexerTest({ status: 'testing' });
    setIndexerTest(await probeEndpoint(`${target.normalized}/health`, null, settings.requestTimeoutMs));
  }, [settings.indexerUrl, settings.requestTimeoutMs]);

  return (
    <>
      <Head>
        <title>Settings - SoroScope</title>
        <meta name="description" content="Configure custom Soroban RPC and indexer endpoints." />
      </Head>

      <main className="min-h-screen bg-slate-950 text-slate-100">
        <div className="mx-auto max-w-3xl px-4 py-8 sm:px-6 lg:px-8">
          <Link
            href="/"
            className="mb-6 inline-flex items-center gap-2 text-sm text-slate-400 transition-colors hover:text-cyan-400"
          >
            <ArrowLeft className="h-4 w-4" />
            Back to explorer
          </Link>

          <h1 className="text-2xl font-bold tracking-tight text-white">Settings</h1>
          <p className="mt-1 text-sm text-slate-400">
            Point SoroScope at your own self-hosted infrastructure. Preferences are stored in this
            browser only and never leave your machine.
          </p>

          <div className="mt-8 space-y-6">
            <EndpointField
              id="rpcUrl"
              label="Custom Soroban RPC endpoint"
              placeholder={network.rpcUrl}
              hint={`Leave blank to use the ${network.shortName} default (${network.rpcUrl}).`}
              value={settings.rpcUrl}
              error={errors.rpcUrl}
              testState={rpcTest}
              onChange={(value) => update('rpcUrl', value)}
              onTest={testRpc}
            />

            <EndpointField
              id="indexerUrl"
              label="Custom indexer / analyzer backend URL"
              placeholder={API_URL}
              hint={`Leave blank to use the built-in backend (${API_URL}).`}
              value={settings.indexerUrl}
              error={errors.indexerUrl}
              testState={indexerTest}
              onChange={(value) => update('indexerUrl', value)}
              onTest={testIndexer}
            />

            <div className="rounded-2xl border border-slate-800 bg-slate-900/70 p-5">
              <label htmlFor="requestTimeoutMs" className="text-sm font-medium text-slate-200">
                Request timeout
              </label>
              <p className="mt-1 text-xs text-slate-500">
                How long to wait for a custom endpoint before giving up (1000–120000&nbsp;ms).
              </p>
              <input
                id="requestTimeoutMs"
                type="number"
                min={1000}
                max={120000}
                step={500}
                value={settings.requestTimeoutMs}
                onChange={(e) => update('requestTimeoutMs', Number(e.target.value))}
                className="mt-3 w-40 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 font-mono text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
              />
            </div>
          </div>

          <div className="mt-8 flex flex-wrap items-center gap-3">
            <button
              type="button"
              onClick={handleSave}
              className="inline-flex min-h-[44px] items-center gap-2 rounded-lg bg-cyan-500 px-4 py-2.5 text-sm font-semibold text-slate-950 transition-colors hover:bg-cyan-400 focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
            >
              <Save className="h-4 w-4" />
              Save preferences
            </button>
            <button
              type="button"
              onClick={handleReset}
              className="inline-flex min-h-[44px] items-center gap-2 rounded-lg border border-slate-700 bg-slate-900 px-4 py-2.5 text-sm font-medium text-slate-300 transition-colors hover:bg-slate-800 hover:text-white focus:outline-none focus:ring-2 focus:ring-cyan-500/50"
            >
              <RotateCcw className="h-4 w-4" />
              Reset to defaults
            </button>
            {saved && (
              <span role="status" className="inline-flex items-center gap-1.5 text-sm text-emerald-400">
                <CheckCircle2 className="h-4 w-4" />
                Preferences saved
              </span>
            )}
          </div>
        </div>
      </main>
    </>
  );
}

interface EndpointFieldProps {
  id: string;
  label: string;
  placeholder: string;
  hint: string;
  value: string;
  error?: string;
  testState: TestState;
  onChange: (value: string) => void;
  onTest: () => void;
}

function EndpointField({
  id,
  label,
  placeholder,
  hint,
  value,
  error,
  testState,
  onChange,
  onTest,
}: EndpointFieldProps) {
  return (
    <div className="rounded-2xl border border-slate-800 bg-slate-900/70 p-5">
      <label htmlFor={id} className="text-sm font-medium text-slate-200">
        {label}
      </label>
      <p className="mt-1 text-xs text-slate-500">{hint}</p>

      <div className="mt-3 flex flex-col gap-2 sm:flex-row">
        <input
          id={id}
          type="url"
          inputMode="url"
          spellCheck={false}
          value={value}
          placeholder={placeholder}
          aria-invalid={Boolean(error)}
          aria-describedby={error ? `${id}-error` : undefined}
          onChange={(e) => onChange(e.target.value)}
          className={`w-full rounded-lg border bg-slate-950 px-3 py-2 font-mono text-sm text-slate-100 focus:outline-none focus:ring-2 ${
            error
              ? 'border-red-500/60 focus:ring-red-500/40'
              : 'border-slate-700 focus:ring-cyan-500/50'
          }`}
        />
        <button
          type="button"
          onClick={onTest}
          disabled={testState.status === 'testing'}
          className="inline-flex min-h-[44px] shrink-0 items-center justify-center gap-2 rounded-lg border border-slate-700 bg-slate-800 px-4 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-60"
        >
          {testState.status === 'testing' ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : null}
          Test
        </button>
      </div>

      {error && (
        <p id={`${id}-error`} role="alert" className="mt-2 text-xs text-red-400">
          {error}
        </p>
      )}

      {testState.status === 'ok' && (
        <p className="mt-2 inline-flex items-center gap-1.5 text-xs text-emerald-400">
          <CheckCircle2 className="h-3.5 w-3.5" />
          {testState.message}
          {testState.latencyMs !== undefined && ` in ${testState.latencyMs}ms`}
        </p>
      )}
      {testState.status === 'fail' && (
        <p className="mt-2 inline-flex items-center gap-1.5 text-xs text-red-400">
          <XCircle className="h-3.5 w-3.5" />
          {testState.message}
        </p>
      )}
    </div>
  );
}
