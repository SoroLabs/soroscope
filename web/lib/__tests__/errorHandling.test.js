const assert = require('node:assert/strict');
const { test, describe } = require('node:test');
const {
  parseContractErrorCode,
  getDescriptiveContractError,
  formatContractErrorMessage,
  createUserFriendlyMessage,
  formatError,
  CONTRACT_ERROR_MAP,
} = require('../errorHandling');

describe('Soroban Contract Error Mapping & Formatting', () => {
  test('parseContractErrorCode extracts integer codes from raw HostError formats', () => {
    assert.equal(parseContractErrorCode('HostError #104'), 104);
    assert.equal(parseContractErrorCode('HostError: Error(Contract, #104)'), 104);
    assert.equal(parseContractErrorCode('Error(Contract, #14)'), 14);
    assert.equal(parseContractErrorCode('Contract execution failed: HostError #3'), 3);
    assert.equal(parseContractErrorCode('Error code: #101'), 101);
    assert.equal(parseContractErrorCode('Error #7'), 7);
    assert.equal(parseContractErrorCode('No error code present here'), null);
  });

  test('getDescriptiveContractError returns corresponding message for known error codes', () => {
    assert.equal(getDescriptiveContractError(1), 'Contract is already initialized');
    assert.equal(getDescriptiveContractError(3), 'Unauthorized access or action');
    assert.equal(getDescriptiveContractError(14), 'Contract operations are paused');
    assert.equal(
      getDescriptiveContractError(104),
      'Unauthorized operation or invalid admin authorization'
    );
    assert.equal(getDescriptiveContractError(9999), null);
  });

  test('formatContractErrorMessage converts raw HostError strings to human-readable strings', () => {
    assert.equal(
      formatContractErrorMessage('HostError #104'),
      'Contract Error #104: Unauthorized operation or invalid admin authorization'
    );
    assert.equal(
      formatContractErrorMessage('HostError: Error(Contract, #14)'),
      'Contract Error #14: Contract operations are paused'
    );
    assert.equal(
      formatContractErrorMessage('HostError #999'),
      'Contract Error #999: Custom contract error 999'
    );
    assert.equal(
      formatContractErrorMessage('Generic network failure'),
      'Generic network failure'
    );
  });

  test('createUserFriendlyMessage converts BackendErrorResponse containing HostError into descriptive message', () => {
    const errorResponse = {
      error: 'BAD_REQUEST',
      message: 'Contract execution failed: HostError #104',
      statusCode: 400,
    };
    const friendlyMsg = createUserFriendlyMessage(errorResponse);
    assert.equal(
      friendlyMsg,
      'Contract Error #104: Unauthorized operation or invalid admin authorization'
    );
  });

  test('formatError formats Error object containing embedded HostError string', () => {
    const err = new Error('Invocation failed with HostError #14');
    const formatted = formatError(err);
    assert.equal(
      formatted.message,
      'Contract Error #14: Contract operations are paused'
    );
  });

  test('CONTRACT_ERROR_MAP covers core contract error codes 1..17', () => {
    for (let code = 1; code <= 17; code++) {
      assert.ok(
        typeof CONTRACT_ERROR_MAP[code] === 'string' && CONTRACT_ERROR_MAP[code].length > 0,
        `Expected error message for contract error code ${code}`
      );
    }
  });
});
