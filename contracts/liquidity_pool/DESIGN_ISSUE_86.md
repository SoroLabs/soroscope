# Issue #86: Multi-Token Basket Liquidity Pool Vault

Work in progress. Planned changes to `contracts/liquidity_pool/`:

1. Generalize pool math from a 2-token pair to an N-token array.
2. Implement a Balancer-style weighted invariant curve.
3. Support single-asset deposit and withdrawal.

Planned verification:

- 3-token pool deposit and swap tests.
- Balance invariant holds after every operation.
