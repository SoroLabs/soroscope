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

export const apiConfig = {
  baseUrl: API_URL,
  environment: process.env.NODE_ENV ?? 'development',
};

export interface ApiRequestOptions extends Omit<RequestInit, 'body'> {
  params?: Record<string, string | number | boolean | null | undefined>;
  token?: string;
  body?: BodyInit | object | null;
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

export function apiUrl(path: string, params?: ApiRequestOptions['params']): string {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  const url = new URL(`${getApiBaseUrl()}${normalizedPath}`);

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
  const { params, token, headers, body, ...requestInit } = options;
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

  const response = await fetch(apiUrl(endpoint, params), {
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
