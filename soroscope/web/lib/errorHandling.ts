/**
 * Error handling utilities for parsing and formatting backend error responses
 */

export interface BackendErrorResponse {
  error: string;
  message: string;
  statusCode?: number;
}

export interface FormattedError {
  type: string;
  message: string;
  details?: string;
  statusCode: number;
  isNetworkError: boolean;
}

/**
 * Extract detailed error information from backend response
 */
export async function extractErrorDetails(response: Response): Promise<BackendErrorResponse> {
  try {
    const data = await response.json();
    return {
      error: data.error || 'UNKNOWN_ERROR',
      message: data.message || response.statusText || 'An error occurred',
      statusCode: response.status,
    };
  } catch {
    // If response body is not JSON, use status text
    return {
      error: getErrorType(response.status),
      message: response.statusText || 'An error occurred',
      statusCode: response.status,
    };
  }
}

/**
 * Map HTTP status codes to error types
 */
function getErrorType(status: number): string {
  switch (status) {
    case 400:
      return 'BAD_REQUEST';
    case 401:
      return 'UNAUTHORIZED';
    case 404:
      return 'NOT_FOUND';
    case 500:
      return 'INTERNAL_SERVER_ERROR';
    case 503:
      return 'SERVICE_UNAVAILABLE';
    default:
      return 'UNKNOWN_ERROR';
  }
}

/**
 * Format error for display
 */
export function formatError(error: unknown): FormattedError {
  if (error instanceof Response) {
    return {
      type: getErrorType(error.status),
      message: error.statusText || 'Network error',
      statusCode: error.status,
      isNetworkError: true,
    };
  }

  if (error instanceof TypeError) {
    return {
      type: 'NETWORK_ERROR',
      message: 'Failed to connect to backend. Please ensure the server is running.',
      details: error.message,
      statusCode: 0,
      isNetworkError: true,
    };
  }

  if (error instanceof Error) {
    // Check if it's a JSON parse error
    if (error.message.includes('JSON')) {
      return {
        type: 'PARSE_ERROR',
        message: 'Failed to parse response from backend',
        details: error.message,
        statusCode: 0,
        isNetworkError: false,
      };
    }

    return {
      type: 'ERROR',
      message: error.message || 'An unexpected error occurred',
      statusCode: 0,
      isNetworkError: false,
    };
  }

  return {
    type: 'UNKNOWN_ERROR',
    message: 'An unexpected error occurred',
    statusCode: 0,
    isNetworkError: false,
  };
}

/**
 * Create a user-friendly error message from BackendErrorResponse
 */
export function createUserFriendlyMessage(errorResponse: BackendErrorResponse): string {
  const errorMessages: Record<string, string> = {
    BAD_REQUEST: 'Invalid request. Please check your inputs and try again.',
    UNAUTHORIZED: 'You are not authorized to perform this action.',
    NOT_FOUND: 'The requested resource was not found.',
    INTERNAL_SERVER_ERROR: 'Server error. Please try again later.',
    SERVICE_UNAVAILABLE: 'The service is currently unavailable. Please try again later.',
  };

  return (
    errorMessages[errorResponse.error] ||
    errorResponse.message ||
    'An error occurred during analysis'
  );
}

/**
 * Categorize and format WASM-specific backend errors
 */
export interface WasmBackendError {
  title: string;
  message: string;
  details?: string;
  suggestedAction?: string;
  statusCode: number;
}

/**
 * Parse WASM-specific errors from backend responses
 */
export function parseWasmError(response: Response, errorMessage: string): WasmBackendError {
  const status = response.status;
  
  // Map common backend error messages to user-friendly ones
  const wasmErrorPatterns: Array<{ pattern: RegExp; title: string; details: (match: RegExpExecArray) => string }> = [
    {
      pattern: /Invalid base64|base64 decoding|base64 WASM data/i,
      title: 'Invalid WASM Encoding',
      details: () => 'The file appears to be corrupted or improperly encoded. Ensure you\'re uploading a valid compiled Soroban contract.',
    },
    {
      pattern: /Invalid WASM|malformed|not a valid WebAssembly/i,
      title: 'Invalid WASM Format',
      details: () => 'This doesn\'t appear to be a valid WebAssembly module. Make sure you\'re uploading a compiled .wasm file from Soroban.',
    },
    {
      pattern: /version|unsupported/i,
      title: 'Unsupported WASM Version',
      details: () => 'The WASM version is not supported. Please recompile using a compatible Soroban version.',
    },
    {
      pattern: /memory|out of|limit|overflow/i,
      title: 'WASM Resource Exceeded',
      details: () => 'The contract exceeds analysis resource limits. Try simplifying the contract or splitting it into smaller modules.',
    },
    {
      pattern: /timeout|took too long|analysis timeout/i,
      title: 'Analysis Timeout',
      details: () => 'The analysis took too long to complete. The contract might be too complex. Please try again or simplify the contract.',
    },
    {
      pattern: /function|export|not found/i,
      title: 'Function Not Found',
      details: () => 'The specified contract function was not found. Ensure the function is properly exported from your contract.',
    },
  ];

  // Check for pattern matches
  for (const { pattern, title, details } of wasmErrorPatterns) {
    const match = pattern.exec(errorMessage);
    if (match) {
      return {
        title,
        message: details(match),
        statusCode: status,
        suggestedAction: 'Please check your contract and try uploading again.',
      };
    }
  }

  // Default mappings by status code
  const defaultErrors: Record<number, WasmBackendError> = {
    400: {
      title: 'Invalid WASM File',
      message: errorMessage || 'The backend rejected the WASM file. Please ensure it\'s a valid compiled Soroban contract.',
      statusCode: 400,
      suggestedAction: 'Try uploading a different contract or check the build logs.',
    },
    401: {
      title: 'Unauthorized',
      message: 'You don\'t have permission to analyze contracts.',
      statusCode: 401,
      suggestedAction: 'Please connect your wallet and try again.',
    },
    413: {
      title: 'File Too Large',
      message: 'The WASM file is too large for analysis.',
      statusCode: 413,
      suggestedAction: 'Optimize your contract to reduce its size.',
    },
    500: {
      title: 'Server Error',
      message: 'The backend encountered an error while analyzing your contract.',
      statusCode: 500,
      suggestedAction: 'Please try again later.',
    },
    503: {
      title: 'Service Unavailable',
      message: 'The analysis service is temporarily unavailable.',
      statusCode: 503,
      suggestedAction: 'Please try again in a few moments.',
    },
  };

  return (
    defaultErrors[status] || {
      title: 'Analysis Failed',
      message: errorMessage || 'An error occurred while analyzing the WASM file.',
      statusCode: status,
      suggestedAction: 'Please try uploading again.',
    }
  );
}
