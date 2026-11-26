# Resource Regression Fixtures

Fixtures use JSON format version `1`. Each document records a WASM path and SHA-256, contract ID, exported function, arguments as base64 XDR `ScVal`s, injected ledger entries as base64 XDR keys and entries, a complete ledger/protocol snapshot, expected `SorobanResources`, and CPU/RAM relative tolerances in basis points.

`resource_fixtures::record_fixture_file` executes the invocation against the local Soroban test host, updates the WASM digest and expected meters, and writes the JSON document. It does not construct or call an RPC client. `resource_fixtures::check_fixture_file` loads the document and relative WASM, rejects a digest mismatch before host execution, replays the invocation from the stored ledger snapshot, and checks the results. CPU and RAM use the configured tolerance; ledger read/write bytes and transaction size are exact.

The helpers are intended to be called from crate tests or a small local recording utility. A recording caller constructs a `ResourceFixture` with the invocation, snapshot, tolerance, and an initial expected value; `record_fixture_file` replaces the digest and expected resources from the local run. Tests should call `check_fixture_file` for each checked-in fixture.

The current local resource profiler reports ledger read/write bytes as zero. Ledger entries are still restored for execution, but meaningful ledger-byte baselines require adding access-meter extraction to the local host instrumentation before fixtures rely on those two fields.
