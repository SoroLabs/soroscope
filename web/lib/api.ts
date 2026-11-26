/**
 * SoroScope API client.
 *
 * Uses the browser-native Fetch API so the frontend does not need an extra
 * Axios dependency. Set NEXT_PUBLIC_API_URL in production to point at the Rust
 * simulation engine backend; local development defaults to localhost.
 */

import type { AnalyzeResponse } from './sorobantypes';
import { loadSettings, resolveEndpoint } from './userSettings';

const DEFAULT_DEV_API_URL = 'http://localhost:8080';

export const API_URL =
  process.env.NEXT_PUBLIC_API_URL?.replace(/\/+$/, '') ?? DEFAULT_DEV_API_URL;

/**
 * Base URL for every backend call.
 *
 * A custom indexer URL saved on the /settings page takes precedence over the
 * build-time `NEXT_PUBLIC_API_URL`, so power users can point the app at their
 * own self-hosted backend without a rebuild. Resolved per request because the
 * preference can change while the app is open.
 */
export function getApiBaseUrl(): string {
  return resolveEndpoint(loadSettings().indexerUrl, API_URL);
}

/**
 * Optional per-network backends for the multi-network comparison view (#1282).
 *
 * The analyzer is configured with a single RPC endpoint at startup, so
 * comparing networks today means running one backend per network and
 * pointing these at them. Unset networks fall back to the default backend,
 * which also receives the `network` field on each request so a server that
 * can switch networks per request works without client changes.
 *
 * Each variable is referenced literally so Next.js can inline it at build.
 */
const NETWORK_API_URLS: Record<string, string | undefined> = {
  mainnet: process.env.NEXT_PUBLIC_API_URL_MAINNET,
  testnet: process.env.NEXT_PUBLIC_API_URL_TESTNET,
  futurenet: process.env.NEXT_PUBLIC_API_URL_FUTURENET,
  localhost: process.env.NEXT_PUBLIC_API_URL_LOCALHOST,
};

export function getNetworkApiBaseUrl(networkId: string): string {
  const override = NETWORK_API_URLS[networkId]?.trim();
  return override ? override.replace(/\/+$/, '') : getApiBaseUrl();
}

export const apiConfig = {
  baseUrl: API_URL,
  environment: process.env.NODE_ENV ?? 'development',
};

export interface ApiRequestOptions extends Omit<RequestInit, 'body'> {
  params?: Record<string, string | number | boolean | null | undefined>;
  token?: string;
  body?: BodyInit | object | null;
  /** Overrides the backend for this request (e.g. a per-network backend). */
  baseUrl?: string;
}

export class ApiError extends Error {
  status: number;
  statusText: string;
  body: unknown;

  constructor(status: number, statusText: string, body: unknown) {
    const message =
      typeof body === 'object' &&
      body !== null &&
      'message' in body &&
      typeof body.message === 'string'
        ? body.message
        : statusText;

    super(`API Error ${status}: ${message}`);
    this.name = 'ApiError';
    this.status = status;
    this.statusText = statusText;
    this.body = body;
  }
}

export function apiUrl(
  path: string,
  params?: ApiRequestOptions['params'],
  baseUrl: string = getApiBaseUrl(),
): string {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  const url = new URL(`${baseUrl}${normalizedPath}`);

  if (params) {
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        url.searchParams.set(key, String(value));
      }
    });
  }

  return url.toString();
}

function isJsonBody(body: ApiRequestOptions['body']): body is object {
  return (
    body !== null &&
    body !== undefined &&
    typeof body === 'object' &&
    !(typeof Blob !== 'undefined' && body instanceof Blob) &&
    !(typeof FormData !== 'undefined' && body instanceof FormData) &&
    !(typeof URLSearchParams !== 'undefined' && body instanceof URLSearchParams) &&
    !(body instanceof ArrayBuffer) &&
    !ArrayBuffer.isView(body)
  );
}

function buildBody(body: ApiRequestOptions['body']): BodyInit | undefined {
  if (body === null || body === undefined) {
    return undefined;
  }

  return isJsonBody(body) ? JSON.stringify(body) : body;
}

async function parseResponse(response: Response): Promise<unknown> {
  if (response.status === 204) {
    return null;
  }

  const contentType = response.headers.get('content-type') ?? '';
  return contentType.includes('application/json') ? response.json() : response.text();
}

async function request<T>(endpoint: string, options: ApiRequestOptions = {}): Promise<T> {
  const { params, token, headers, body, baseUrl, ...requestInit } = options;
  const requestHeaders = new Headers(headers);

  if (!requestHeaders.has('Accept')) {
    requestHeaders.set('Accept', 'application/json');
  }

  if (token && !requestHeaders.has('Authorization')) {
    requestHeaders.set('Authorization', `Bearer ${token}`);
  }

  if (isJsonBody(body) && !requestHeaders.has('Content-Type')) {
    requestHeaders.set('Content-Type', 'application/json');
  }

  const response = await fetch(apiUrl(endpoint, params, baseUrl), {
    ...requestInit,
    headers: requestHeaders,
    body: buildBody(body),
  });

  const responseBody = await parseResponse(response);

  if (!response.ok) {
    throw new ApiError(response.status, response.statusText, responseBody);
  }

  return responseBody as T;
}

export const apiClient = {
  request,

  get<T>(endpoint: string, options?: ApiRequestOptions): Promise<T> {
    return request<T>(endpoint, { ...options, method: 'GET' });
  },

  post<T>(endpoint: string, body?: ApiRequestOptions['body'], options?: ApiRequestOptions): Promise<T> {
    return request<T>(endpoint, { ...options, method: 'POST', body });
  },

  put<T>(endpoint: string, body?: ApiRequestOptions['body'], options?: ApiRequestOptions): Promise<T> {
    return request<T>(endpoint, { ...options, method: 'PUT', body });
  },

  patch<T>(endpoint: string, body?: ApiRequestOptions['body'], options?: ApiRequestOptions): Promise<T> {
    return request<T>(endpoint, { ...options, method: 'PATCH', body });
  },

  delete<T>(endpoint: string, options?: ApiRequestOptions): Promise<T> {
    return request<T>(endpoint, { ...options, method: 'DELETE' });
  },
};

export interface AnalyzeRequest {
  contract_id: string;
  function_name: string;
  args?: string[];
  ledger_overrides?: Record<string, string>;
  protocol_version?: number;
  enable_experimental?: boolean;
  /** Target network id (e.g. `testnet`, `futurenet`). */
  network?: string;
}

export interface AnalyzeWasmRequest {
  wasm_bytes: string;
  function_name: string;
  args?: string[];
  protocol_version?: number;
  enable_experimental?: boolean;
}

import { simulationQueueManager } from './requestQueue';

export const analyzeService = {
  analyze(req: AnalyzeRequest, token?: string): Promise<AnalyzeResponse> {
    return simulationQueueManager.enqueue(() =>
      apiClient.post<AnalyzeResponse>('/analyze', req, { token }),
    );
  },

  analyzeWasm(req: AnalyzeWasmRequest, token?: string): Promise<AnalyzeResponse> {
    return simulationQueueManager.enqueue(() =>
      apiClient.post<AnalyzeResponse>('/analyze/wasm', req, { token }),
    );
  },
};

export interface NetworkAnalysisOutcome {
  networkId: string;
  report?: AnalyzeResponse;
  error?: string;
}

/**
 * Run the same simulation on several networks at once (#1282). Requests go
 * out in parallel; one network failing does not hide the others' results.
 */
export async function analyzeAcrossNetworks(
  req: Omit<AnalyzeRequest, 'network'>,
  networkIds: string[],
  token?: string,
): Promise<NetworkAnalysisOutcome[]> {
  const settled = await Promise.allSettled(
    networkIds.map((networkId) =>
      apiClient.post<AnalyzeResponse>(
        '/analyze',
        { ...req, network: networkId },
        { token, baseUrl: getNetworkApiBaseUrl(networkId) },
      ),
    ),
  );

  return settled.map((outcome, i) =>
    outcome.status === 'fulfilled'
      ? { networkId: networkIds[i], report: outcome.value }
      : {
          networkId: networkIds[i],
          error:
            outcome.reason instanceof Error
              ? outcome.reason.message
              : 'Simulation failed',
        },
  );
}

/**
 * Persisted simulation results (#1286). The server stores every completed
 * simulation and returns its id as `simulation_id`; this fetches one back.
 */
export const simulationService = {
  get(simulationId: string, token?: string): Promise<unknown> {
    return apiClient.get<unknown>(
      `/api/simulations/${encodeURIComponent(simulationId)}`,
      { token },
    );
  },
};
