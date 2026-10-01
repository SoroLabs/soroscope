use serde::{Deserialize, Serialize};

/// OpenAPI document for the comparison API.
///
/// The comparison endpoint returns either a percentage delta or an explicit
/// `incomparable` result. Two runs are incomparable when their cost parameter
/// hashes differ, because a percentage delta across fee schedules is meaningless.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ComparisonResponse {
    /// `ok` when a delta was computed, `incomparable` when the runs cannot be
    /// compared.
    pub status: ComparisonStatus,
    /// Percentage delta for CPU instructions. Only present when `status == ok`.
    #[serde(skip_serializing_if_none)]
    pub cpu_delta_percent: Option<f64>,
    /// Percentage delta for ledger bytes. Only present when `status == ok`.
    #[serde(skip_serializing_if_none)]
    pub ledger_delta_percent: Option<f64>,
    /// Human-readable reason. Only present when `status == incomparable`.
    ///
    /// The canonical reason is `"cost_params_hash differs"`.
    #{serde(skip_serializing_if_none)}]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonStatus {
    Ok,
    Incomparable,
}

/// OpenAPI schema document in JSON Schema form.
///
/// The comparison response documents the `incomparable` case explicitly:
/// when `cost_params_hash` differs, no percentage delta is returned and
/// the `reason` field is populated instead.
///
/// # Example
///
/// ```json
/// {
///   "status": "incomparable",
///   "reason": "cost_params_hash differs",
///   "cpu_delta_percent": null,
///   "ledger_delta_percent": null
/// }
/// ```
pub const COMPARISON_RESPONSE_SCHEMA: &'static str = r