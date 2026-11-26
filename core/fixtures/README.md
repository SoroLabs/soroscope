# Hermetic Resource Fixtures

Each JSON file in this directory is checked locally by the `resource-fixtures` binary. Its `wasm_path` is relative to this directory, so checked-in fixture WASM files should live beside the JSON document or point at the reproducible release artifact copied into this directory by CI.

Use `resource-fixtures check --refresh` only from a pull request carrying the `baseline-update` label. The workflow uses that label to permit refreshes and still reports the old-baseline delta for review. Normal checks never rewrite fixtures.