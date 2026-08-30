const test = require('node:test');
const assert = require('node:assert/strict');

// SEO logic mapping simulated for unit assertions
function getMetadata(tab, selectedFunction, contractId) {
  switch (tab) {
    case 'analytics':
      return {
        pageTitle: 'SoroScope | Liquidity Pool APY & TVL Analytics',
        seoDescription: 'Explore historical APY, TVL, and volume charts for the XLM/USDC liquidity pool.',
      };
    case 'transactions':
      return {
        pageTitle: 'SoroScope | Transaction History Telemetry',
        seoDescription: 'Monitor real-time Soroban contract events, transaction fees, and telemetry records.',
      };
    case 'history':
      return {
        pageTitle: 'SoroScope | Invocation History Analysis',
        seoDescription: 'Review previous Soroban contract runs and CPU/RAM instruction summaries.',
      };
    case 'explorer':
    default:
      return {
        pageTitle: `SoroScope | ${selectedFunction.name} - Contract Analyzer`,
        seoDescription: `Analyze CPU, RAM, and ledger footprint of the ${selectedFunction.name} function on contract ${contractId}.`,
      };
  }
}

test('SEO Metadata: generates custom title and description for explorer tab', () => {
  const result = getMetadata('explorer', { name: 'transfer' }, 'CC123');
  assert.strictEqual(result.pageTitle, 'SoroScope | transfer - Contract Analyzer');
  assert.strictEqual(result.seoDescription, 'Analyze CPU, RAM, and ledger footprint of the transfer function on contract CC123.');
});

test('SEO Metadata: generates static title and description for analytics tab', () => {
  const result = getMetadata('analytics');
  assert.strictEqual(result.pageTitle, 'SoroScope | Liquidity Pool APY & TVL Analytics');
  assert.ok(result.seoDescription.includes('historical APY, TVL'));
});

test('SEO Metadata: generates static title and description for transactions tab', () => {
  const result = getMetadata('transactions');
  assert.strictEqual(result.pageTitle, 'SoroScope | Transaction History Telemetry');
  assert.ok(result.seoDescription.includes('Monitor real-time Soroban contract events'));
});

test('SEO Metadata: generates static title and description for history tab', () => {
  const result = getMetadata('history');
  assert.strictEqual(result.pageTitle, 'SoroScope | Invocation History Analysis');
  assert.ok(result.seoDescription.includes('Review previous Soroban contract runs'));
});
