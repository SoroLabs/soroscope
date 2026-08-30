# Security Audit Checklist — Production Deployment

This checklist must be reviewed and signed off before deploying any SoroScope contract to Stellar Mainnet.

---

## Admin Key Management

- [ ] Admin secret keys are stored in a hardware security module (HSM) or equivalent secure vault — never in plaintext files or environment variables on shared systems.
- [ ] The deployer secret key used during initial deployment is rotated immediately after deployment and the post-deployment admin key is set to the long-term multisig address.
- [ ] No single person holds all signing keys for the multisig admin address.
- [ ] Admin key rotation procedures are documented, tested on Testnet, and accessible to at least two team members.
- [ ] CI/CD pipeline secrets (`TESTNET_DEPLOYER_SECRET_KEY` etc.) are scoped to their target environment and rotated on a defined schedule.

---

## Multisig Thresholds

- [ ] All privileged operations (pause, unpause, admin rotation, emergency withdrawal) require M-of-N multisig approval where N ≥ 3 and M ≥ 2.
- [ ] Multisig thresholds are validated on-chain — not just enforced by off-chain tooling.
- [ ] The multisig configuration has been verified with the `scripts/audit_multisig_thresholds.py` script against the deployed contract state.
- [ ] Threshold values are documented and match what was reviewed during audit.

---

## EmergencyGuard Configuration

- [ ] Every deployed contract that handles user funds integrates `EmergencyGuard` (see [`contracts/emergency_guard/`](../contracts/emergency_guard/)).
- [ ] Granular pause flags are configured for each sensitive operation (swap, deposit, withdrawal, transfer, mint, burn).
- [ ] The initial pause state is verified to be unpaused (all bits = 0) before going live — unless a guarded rollout is intended.
- [ ] An emergency pause runbook exists and is reachable by on-call engineers without needing repository access.
- [ ] The `guard_unpause` path has been tested on Testnet under simulated incident conditions.

---

## Access Controls & Limits

- [ ] All admin-only entry points are protected by an `require_auth` check against the stored admin address.
- [ ] Fee parameters have hard-coded maximum caps that cannot be overridden by admin calls alone.
- [ ] Any upgrade authority (if applicable) is locked to the multisig address.
- [ ] There are no backdoor or debug entry points left active in the deployed WASM.

---

## Contract Deployment

- [ ] WASM artifacts were compiled from a tagged release commit — not a development branch.
- [ ] The deployed WASM hash has been verified against the locally-built artifact (`sha256sum`).
- [ ] Contract IDs are recorded in a deployment manifest and stored in version control (see `deployment_manifest.txt` produced by the CI deploy job).
- [ ] All constructor arguments (admin address, fee bps, limits) have been double-checked against the final deployment parameters doc.

---

## Cross-Chain Components

- [ ] Merkle roots posted via `update_root` are only accepted from an authorized relayer address.
- [ ] The relayer key is separate from the admin key and has no pause/admin authority.
- [ ] `verify_message` proofs have been tested end-to-end on Testnet before enabling the Mainnet relayer.
- [ ] A maximum batch size or rate limit is in place for root updates to prevent spam.

---

## Dependency & Build Review

- [ ] `Cargo.lock` is committed and pinned — no wildcard version ranges for security-sensitive crates.
- [ ] `cargo audit` has been run and all advisories reviewed (run `cargo install cargo-audit && cargo audit`).
- [ ] The Soroban SDK version matches the version targeted in the audit report.
- [ ] No `unsafe` blocks are present in contract code (verify with `grep -r "unsafe" contracts/`).

---

## Incident Response

- [ ] An on-call rotation is in place before Mainnet launch.
- [ ] The emergency pause key is accessible 24/7 to at least one on-call engineer.
- [ ] A post-incident review process is defined and communicated to the team.
- [ ] Contact information for the Stellar Foundation security team is documented for coordinated disclosure.

---

## Sign-off

| Role | Name | Date |
|------|------|------|
| Lead Engineer | | |
| Security Reviewer | | |
| Multisig Key Holder 1 | | |
| Multisig Key Holder 2 | | |
</content>
</invoke>