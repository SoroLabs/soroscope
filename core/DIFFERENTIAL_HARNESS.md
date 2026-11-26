# Differential Simulation Harness

The harness runs the same WASM, contract ID, function, arguments, and protocol version through the local host and RPC independently. It does not fall back from one side to the other; `DifferentialReport` retains both outcomes, including one-sided failures.

CPU and memory relative error are checked against the checked-in protocol snapshot in `config/differential-tolerances.json`. Tolerance changes are fixture changes and should be made only when explicitly refreshing the protocol snapshot, not as part of a feature change.

Ledger read/write bytes are compared exactly only when `same_ledger_entries_confirmed` is true. The public Stellar `simulateTransaction` request accepts a transaction, not arbitrary ledger-entry overrides, so the RPC node must already be pinned to the same ledger fixture for that assertion to be valid. The ignored live test currently leaves this flag false; it measures CPU and memory against the configured network without claiming ledger-state parity.

## Ignored RPC Test

The integration test is ignored by default so normal CI remains hermetic. Configure these variables and run `cargo test -p soroscope-core --test differential_rpc -- --ignored` from the workspace root:

- `DIFFERENTIAL_RPC_URL`: Soroban RPC endpoint.
- `DIFFERENTIAL_CONTRACT_ID`: deployed contract matching the supplied WASM.
- `DIFFERENTIAL_WASM_PATH`: local WASM file for that deployed contract.
- `DIFFERENTIAL_FUNCTION`: exported function to invoke.
- `DIFFERENTIAL_ARGS_JSON`: optional JSON string array of arguments; defaults to `[]`.
- `DIFFERENTIAL_PROTOCOL_VERSION`: protocol version; defaults to `22` and must have a checked-in tolerance snapshot.