# Cryptographic Assumptions & Privacy Architecture

This document describes the cryptographic foundations, threat model, and privacy guarantees of the Pedersen commitment scheme implemented in `contracts/private_transfer`.

---

## 1. Mathematical Formulation

The private transfer contract replaces plaintext on-chain asset amounts with **Pedersen commitments** over a fixed elliptic curve group:

$$C = v \cdot G + r \cdot H$$

Where:
- $v \in [0, 2^{64}-1]$: The confidential transfer amount ($v \ge 0$).
- $r \in \mathbb{F}_p$: The secret blinding factor chosen uniformly at random off-chain.
- $G \in E(\mathbb{F}_p)$: Primary curve base generator point.
- $H \in E(\mathbb{F}_p)$: Secondary generator point chosen such that $\log_G(H)$ is computationally infeasible to determine.
- $C \in E(\mathbb{F}_p)$: Resulting commitment point, serialized into a canonical 32-byte representation (`BytesN<32>`).

Only the commitment $C$ is stored or transmitted on-chain in contract storage; the actual transaction amount $v$ and blinding factor $r$ remain strictly private off-chain.

---

## 2. Fixed Elliptic Curve Parameters

The contract defines a fixed prime-order Weierstrass elliptic curve:

$$E: y^2 \equiv x^3 + 7x + 11 \pmod p$$

- **Field Characteristic**:
  $$p = 2^{64} - 59 = 18446744073709551557$$
  A 64-bit prime providing efficient, overflow-free arithmetic natively in WebAssembly / Soroban without external non-standard bigint crates.
- **Discriminant**:
  $$\Delta = -16(4a^3 + 27b^2) = -16(4(7)^3 + 27(11)^2) \not\equiv 0 \pmod p$$
  ensuring $E$ is non-singular.
- **Generator Point $G$**:
  $$G = (3, 4294967296) = (3, 2^{32})$$
- **Secondary Generator Point $H$**:
  $$H = (102, 10432324945265058214)$$
  Derived via nothing-up-my-sleeve point selection to guarantee that $\log_G(H)$ is unknown.

---

## 3. Cryptographic Assumptions

The security of this scheme relies on two standard hardness assumptions:

1. **Discrete Logarithm (DLOG) Assumption**:
   For randomly chosen points $G, H \in E(\mathbb{F}_p)$, no polynomial-time algorithm can compute $x \in \mathbb{F}_p$ such that $H = x \cdot G$ with non-negligible probability.
2. **Computational Diffie-Hellman (CDH) Assumption**:
   Given $G, aG, bG$, computing $abG$ is intractable.

---

## 4. Security Guarantees

### Perfect (Information-Theoretic) Hiding
For any amount $v$, as long as the blinding factor $r$ is chosen uniformly at random from $\mathbb{F}_p$, the resulting commitment $C = vG + rH$ is uniformly distributed over the curve group. An adversary with infinite computing power cannot extract any information about the transaction amount $v$ from $C$.

### Computational Binding
A committer cannot open a commitment $C$ to two distinct values $(v, r) \neq (v', r')$.
If an adversary could find $(v, r)$ and $(v', r')$ such that:
$$vG + rH = v'G + r'H$$
Then:
$$(v - v')G = (r' - r)H \implies H = \frac{v - v'}{r' - r} G$$
This would immediately solve the Discrete Logarithm Problem $\log_G(H)$, contradicting the DLOG assumption.

### Additive Homomorphism
Pedersen commitments support homomorphic combination:
$$C(v_1, r_1) + C(v_2, r_2) = (v_1 + v_2)G + (r_1 + r_2)H = C(v_1 + v_2, r_1 + r_2)$$
This property allows off-chain balance reconciliation and zero-knowledge batch proofs without disclosing individual account balances.

---

## 5. Verification Interface

The verifier accepts a commitment and an opening:
```rust
pub struct CommitmentOpening {
    pub amount: i128,
    pub blinding_factor: BytesN<32>,
}
```

- **`store_commitment(env, commitment)`**: Persists an on-chain commitment without revealing the secret value or blinding factor.
- **`verify_opening(env, commitment, opening)`**: Verifies that:
  1. The commitment $C$ is registered in contract storage.
  2. The opening $(v, r)$ evaluates exactly to $C = vG + rH$.
  3. The amount $v \ge 0$ is non-negative.
