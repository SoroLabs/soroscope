'use strict';
/**
 * Error handling utilities for parsing and formatting backend error responses
 * and Soroban contract error codes.
 *
 * CommonJS mirror of errorHandling.ts — used by Node test runner tests in
 * lib/__tests__/errorHandling.test.js.
 */

/**
 * Lookup table converting Soroban contract error codes (integers) to user-friendly messages.
 */
const CONTRACT_ERROR_MAP = Object.freeze({
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
});

/**
 * Extract contract error code integer from raw error string.
 * @param {string} message
 * @returns {number|null}
 */
function parseContractErrorCode(message) {
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
      if (!isNaN(code)) return code;
    }
  }

  return null;
}

/**
 * Convert a contract error integer to a descriptive human-readable message.
 * @param {number} code
 * @returns {string|null}
 */
function getDescriptiveContractError(code) {
  return CONTRACT_ERROR_MAP[code] || null;
}

/**
 * Parse raw error message and format any embedded contract error codes.
 * @param {string} message
 * @returns {string}
 */
function formatContractErrorMessage(message) {
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
 * Map HTTP status codes to error type strings.
 * @param {number} status
 * @returns {string}
 */
function getErrorType(status) {
  switch (status) {
    case 400: return 'BAD_REQUEST';
    case 401: return 'UNAUTHORIZED';
    case 404: return 'NOT_FOUND';
    case 500: return 'INTERNAL_SERVER_ERROR';
    case 503: return 'SERVICE_UNAVAILABLE';
    default:  return 'UNKNOWN_ERROR';
  }
}

/**
 * Format an error value for display.
 * @param {unknown} error
 * @returns {{ type: string, message: string, details?: string, statusCode: number, isNetworkError: boolean }}
 */
function formatError(error) {
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
 * Create a user-friendly error message from a BackendErrorResponse object.
 * @param {{ error: string, message: string, statusCode?: number }} errorResponse
 * @returns {string}
 */
function createUserFriendlyMessage(errorResponse) {
  if (errorResponse.message) {
    const formatted = formatContractErrorMessage(errorResponse.message);
    if (formatted !== errorResponse.message) {
      return formatted;
    }
  }

  const errorMessages = {
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

module.exports = {
  CONTRACT_ERROR_MAP,
  parseContractErrorCode,
  getDescriptiveContractError,
  formatContractErrorMessage,
  formatError,
  createUserFriendlyMessage,
  getErrorType,
};
