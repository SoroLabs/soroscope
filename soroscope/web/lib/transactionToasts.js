function buildTransactionToast(phase, options = {}) {
  const title = getTransactionToastTitle(phase);
  const message = getTransactionToastMessage(phase, options);

  return {
    id: `${phase}-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    phase,
    title,
    message,
    txHash: options.txHash,
  };
}

function getTransactionToastTitle(phase) {
  switch (phase) {
    case 'signing':
      return 'Signing...';
    case 'submitting':
      return 'Submitting...';
    case 'success':
      return 'Success';
    case 'failed':
      return 'Failed';
    default:
      return 'Transaction';
  }
}

function getTransactionToastMessage(phase, options = {}) {
  switch (phase) {
    case 'signing':
      return options.message || 'Please review and approve the transaction in your wallet.';
    case 'submitting':
      return options.message || 'The transaction is being broadcast to the network.';
    case 'success':
      if (options.txHash) {
        return `Transaction confirmed. Hash: ${options.txHash}`;
      }
      return options.message || 'The transaction completed successfully.';
    case 'failed':
      return options.message || 'The transaction could not be completed.';
    default:
      return options.message || 'Transaction update';
  }
}

module.exports = {
  buildTransactionToast,
  getTransactionToastTitle,
  getTransactionToastMessage,
};
