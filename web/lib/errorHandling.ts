/**
 * Error handling utilities for parsing and formatting backend error responses
 * and Soroban contract error codes.
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
 * Lookup table converting Soroban contract error codes (integers) to user-friendly messages.
 * Maps standard contract error codes and domain-specific error numbers to descriptive text.
 */
export const CONTRACT_ERROR_MAP: Record<number, string> = {
  // Standard Soroban Contract Error Codes (1..17)
  1: 'Contract is already initialized',
  2: 'Contract is not initialized',
  3: 'Unauthorized access or action',
  4: 'Insufficient token balance',
  5: 'Insufficient pool liquidity',
  6: 'Insufficient LP shares',
  7: 'Insufficient token allowance',
  8: 'Slippage tolerance exceeded',
  9: 'Invalid fee parameter',
  10: 'No pending fee update',
  11: 'Timelock delay has not elapsed',
  12: 'Oracle source not configured',
  13: 'Invalid or stale oracle price',
  14: 'Contract operations are paused',
  15: 'Arithmetic overflow in contract math',
  16: 'Division by zero',
  17: 'Invalid input arguments',

  // Common Domain / Custom Soroban Contract Error Codes (100..120)
  100: 'Account or position not found',
  101: 'Invalid contract state or configuration',
  102: 'Operation deadline has expired',
  103: 'Transaction resource limit exceeded',
  104: 'Unauthorized operation or invalid admin authorization',
  105: 'Contract is locked or emergency paused',
  106: 'Undercollateralized position or insufficient collateral ratio',
  107: 'Flash loan repayment failed or vault balance mismatch',
  108: 'Invalid signature or authorization payload',
  109: 'Maximum deposit or mint limit exceeded',
  110: 'Auction or offer has ended',
};

/**
 * Extract contract error code integer from raw error string (e.g. "HostError #104", "Error(Contract, #104)", "#104").
 */
export function parseContractErrorCode(message: string): number | null {
  if (!message || typeof message !== 'string') return null;

  const patterns = [
    /HostError\s*#(\d+)/i,
    /Error\s*\(\s*Contract\s*,\s*#?(\d+)\s*\)/i,
    /Error\s*\(\s*Value\s*,\s*#?(\d+)\s*\)/i,
    /ContractError\s*#?(\d+)/i,
    /Contract\s*Error\s*#?(\d+)/i,
    /Error\s*code\s*:?\s*#?(\d+)/i,
    /Error\s*#(\d+)/i,
    /Code\s*#?(\d+)/i,
    /#(\d+)/,
  ];

  for (const pattern of patterns) {
    const match = message.match(pattern);
    if (match && match[1]) {
      const code = parseInt(match[1], 10);
      if (!isNaN(code)) {
        return code;
      }
    }
  }

  return null;
}

/**
 * Convert a contract error integer to a descriptive human-readable message.
 */
export function getDescriptiveContractError(code: number): string | null {
  return CONTRACT_ERROR_MAP[code] || null;
}

/**
 * Parse raw error message and format any embedded contract error codes into a descriptive user message.
 */
export function formatContractErrorMessage(message: string): string {
  if (!message) return 'An unexpected error occurred';

  const code = parseContractErrorCode(message);
  if (code !== null) {
    const descriptive = getDescriptiveContractError(code);
    if (descriptive) {
      return `Contract Error #${code}: ${descriptive}`;
    }
    return `Contract Error #${code}: Custom contract error ${code}`;
  }

  return message;
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

    const formattedMessage = formatContractErrorMessage(error.message);

    return {
      type: 'ERROR',
      message: formattedMessage || error.message || 'An unexpected error occurred',
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
  if (errorResponse.message) {
    const formattedContractError = formatContractErrorMessage(errorResponse.message);
    if (formattedContractError !== errorResponse.message) {
      return formattedContractError;
    }
  }

  const errorMessages: Record<string, string> = {
    BAD_REQUEST: 'Invalid request. Please check your inputs and try again.',
    UNAUTHORIZED: 'You are not authorized to perform this action.',
    NOT_FOUND: 'The requested resource was not found.',
    INTERNAL_SERVER_ERROR: 'Server error. Please try again later.',
    SERVICE_UNAVAILABLE: 'The service is currently unavailable. Please try again later.',
  };

  if (errorResponse.message && !errorMessages[errorResponse.message]) {
    return formatContractErrorMessage(errorResponse.message);
  }

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
