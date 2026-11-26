use soroscope_core::differential::{DifferentialInput, DifferentialRunner, MeterTolerances};
use soroscope_core::runner::{default_ledger_info, LocalRunner};
use std::sync::Arc;

#[tokio::test]
#[ignore = "requires a deployed matching contract and configured Soroban RPC"]
async fn differential_matches_configured_rpc() {
    let rpc_url = std::env::var("DIFFERENTIAL_RPC_URL").expect("DIFFERENTIAL_RPC_URL is required");
    let contract_id =
        std::env::var("DIFFERENTIAL_CONTRACT_ID").expect("DIFFERENTIAL_CONTRACT_ID is required");
    let wasm_path =
        std::env::var("DIFFERENTIAL_WASM_PATH").expect("DIFFERENTIAL_WASM_PATH is required");
    let function_name =
        std::env::var("DIFFERENTIAL_FUNCTION").expect("DIFFERENTIAL_FUNCTION is required");
    let args = serde_json::from_str(
        &std::env::var("DIFFERENTIAL_ARGS_JSON").unwrap_or_else(|_| "[]".to_string()),
    )
    .expect("DIFFERENTIAL_ARGS_JSON must be a JSON string array");
    let protocol_version = std::env::var("DIFFERENTIAL_PROTOCOL_VERSION")
        .unwrap_or_else(|_| "22".to_string())
        .parse::<u32>()
        .expect("DIFFERENTIAL_PROTOCOL_VERSION must be an integer");

    let report =
        DifferentialRunner::new(rpc_url, Arc::new(LocalRunner::new(default_ledger_info())))
            .run(DifferentialInput {
                wasm: std::fs::read(wasm_path).expect("failed to read DIFFERENTIAL_WASM_PATH"),
                contract_id,
                function_name,
                args,
                protocol_version,
                same_ledger_entries_confirmed: false,
            })
            .await
            .expect("differential report could not be produced");

    assert!(
        report.passed,
        "differential mismatch: {:?}",
        report.failures
    );
    let _ = MeterTolerances::for_protocol(protocol_version).expect("missing checked-in tolerances");
}
