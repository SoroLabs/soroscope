# Contributing to SoroScope

Thank you for your interest in contributing to **SoroScope**! We are excited to have you as part of our community.

As a project in the **Stellar Wave Program**, we value collaboration, clear communication, and reliable engineering practices. Please use the guides below to get started.

## Guides

- [**Development & Setup**](./docs/development.md): Learn how to set up the monorepo and follow the project's coding standards.
- [**How to Open a Pull Request**](./docs/pull-requests.md): Follow the step-by-step workflow for submitting code.
- [**Reporting Issues**](./docs/issues.md): Report bugs or suggest new features.

## Mobile Viewport Checklist

To prevent horizontal overflow on mobile devices, follow these steps when changing responsive layouts:

1. Add `max-w-full overflow-x-auto` to resource data-table containers where horizontal scrolling is appropriate.
2. Wrap responsive flex grids with suitable breakpoint utility classes.
3. Test the layout at a 375px viewport width.

### Acceptance Criteria

- Inspect the site using mobile view mode in browser DevTools.
- Confirm that the page has no horizontal body scrollbar.
- Confirm that tables and other wide components remain usable on small screens.

## Fuzz Testing

SoroScope includes Cargo Fuzz targets for testing the XDR decoder and WebAssembly (WASM) parser against malformed and unexpected inputs. Fuzzing helps identify crashes and panics that ordinary unit tests may not cover.

### Prerequisites

Install the nightly Rust toolchain and `cargo-fuzz`:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz --locked
```

You may also need the native build tools required by `libfuzzer-sys`, including Clang.

### Run the Fuzz Targets Locally

From the repository root, change to the `core` directory:

```bash
cd core
```

Run the XDR decoder target:

```bash
cargo +nightly fuzz run fuzz_xdr_decoder
```

Run the WASM parser target:

```bash
cargo +nightly fuzz run fuzz_wasm_parser
```

To run each target for 60 seconds, matching the CI time limit:

```bash
cargo +nightly fuzz run fuzz_xdr_decoder -- -max_total_time=60
cargo +nightly fuzz run fuzz_wasm_parser -- -max_total_time=60
```

### Interpreting Results

Malformed input that is rejected with an ordinary parsing error is expected. A panic, crash, or sanitizer-detected fault should be treated as a failure and investigated.

Cargo Fuzz stores crash artifacts under:

```text
core/fuzz/artifacts/<target>/
```

Corpus inputs are stored under:

```text
core/fuzz/corpus/<target>/
```

If a crash reveals a bug:

1. Preserve the crashing input so the issue can be reproduced.
2. Minimize the input when possible.
3. Add a regression test for the failure.
4. Fix the underlying defect and rerun the relevant fuzz target.

Do not commit large generated corpora or crash artifacts unless they are intentionally curated as regression fixtures.

Fuzzing complements, but does not replace, the standard project checks. Run the applicable checks for your changes, for example:

```bash
cargo check -p soroscope-core
cargo clippy -p soroscope-core --all-targets --all-features -- -D warnings
cargo test -p soroscope-core
```

### CI Fuzzing

The CI workflow runs `fuzz_xdr_decoder` and `fuzz_wasm_parser` for 60 seconds each. A panic or crash detected during either campaign should fail that CI step.

## Questions?

Feel free to open an **Issue** or reach out to the **SoroLabs** team. Let's build the best Soroban developer tools together!
