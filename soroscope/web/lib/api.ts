export interface ApiRequestOptions extends RequestInit {
  params?: Record<string, string | number | boolean | null | undefined>;
  token?: string;
}

export class ApiError extends Error {
  status: number;
  statusText: string;
  body: unknown;

  constructor(status: number, statusText: string, body: unknown) {
    super(`API Error ${status}: ${body && typeof body === 'object' && 'message' in body ? (body as { message?: string }).message : statusText}`);
    this.name = 'ApiError';
    this.status = status;
    this.statusText = statusText;
    this.body = body;
  }
}

const BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080';

async function request<T>(endpoint: string, options: ApiRequestOptions = {}): Promise<T> {
  const { params, token, headers, ...customConfig } = options;

  const searchParams = new URLSearchParams();
  if (params) {
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        searchParams.set(key, String(value));
      }
    });
  }

  const fullUrl = `${BASE_URL}${endpoint}${searchParams.toString() ? `?${searchParams.toString()}` : ''}`;

  const defaultHeaders: Record<string, string> = {
    'Content-Type': 'application/json',
    Accept: 'application/json',
  };

  if (token) {
    defaultHeaders.Authorization = `Bearer ${token}`;
  }

  const config: RequestInit = {
    method: options.method || 'GET',
    headers: {
      ...defaultHeaders,
      ...headers,
    },
    ...customConfig,
  };

  const response = await fetch(fullUrl, config);
  const contentType = response.headers.get('content-type') ?? '';
  const responseData = contentType.includes('application/json') ? await response.json() : await response.text();

  if (!response.ok) {
    throw new ApiError(response.status, response.statusText, responseData);
  }

  return responseData as T;
}

export const apiClient = {
  get<T>(endpoint: string, options?: ApiRequestOptions): Promise<T> {
    return request<T>(endpoint, { ...options, method: 'GET' });
  },
  post<T>(endpoint: string, body?: unknown, options?: ApiRequestOptions): Promise<T> {
    return request<T>(endpoint, {
      ...options,
      method: 'POST',
      body: body ? JSON.stringify(body) : undefined,
    });
  },
  put<T>(endpoint: string, body?: unknown, options?: ApiRequestOptions): Promise<T> {
    return request<T>(endpoint, {
      ...options,
      method: 'PUT',
      body: body ? JSON.stringify(body) : undefined,
    });
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

export const analyzeService = {
  analyze(req: AnalyzeRequest, token?: string): Promise<any> {
    return apiClient.post('/analyze', req, { token });
  },
  analyzeWasm(req: AnalyzeWasmRequest, token?: string): Promise<any> {
    return apiClient.post('/analyze/wasm', req, { token });
  },
};
