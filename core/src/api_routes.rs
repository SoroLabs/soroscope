//! HTTP surface for the resource-fee quote and SAC transfer profiling.
//!
//! These live in the library rather than in the binary so they can be unit
//! tested without standing up the whole server, and so the binary only has to
//! merge the routers:
//!
//! ```ignore
//! app = app
//!     .merge(soroscope_core::api_routes::fee_quote_routes(engine.clone()))
//!     .merge(soroscope_core::api_routes::sac_transfer_routes(engine.clone()));
//! ```

use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::errors::AppError;
use crate::fee_quote::{self, DurabilitySplit, FeeQuoteInput, ResourceFeeQuote, SorobanFeeConfig};
use crate::sac_transfer::{
    self, SacBalanceSource, SacBalances, SacTransferReport, SacTransferRequest,
};
use crate::simulation::SimulationEngine;
use crate::webhooks::{
    DlqEntry, ReplayOutcome, SubscriptionRegistry, WebhookConfig, WebhookDlq, WebhookError,
};

/// `GET /fee/config` — the checked-in fee parameters a quote is based on.
///
/// Exposed so a client can show *why* a number came out the way it did, and
/// notice when the checked-in snapshot is stale.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FeeConfigResponse {
    pub config: SorobanFeeConfig,
}

/// Request body for `POST /fee/quote`.
///
/// Takes the measured resources explicitly rather than a contract id, so a
/// quote can be recomputed for an already-cached simulation without paying for
/// another round trip to the node.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeeQuoteRequest {
    #[serde(default)]
    pub cpu_instructions: u64,
    #[serde(default)]
    pub ledger_read_bytes: u64,
    #[serde(default)]
    pub ledger_write_bytes: u64,
    #[serde(default)]
    pub transaction_size_bytes: u64,
    #[serde(default)]
    pub read_entries: u64,
    #[serde(default)]
    pub write_entries: u64,
    #[serde(default)]
    pub contract_event_bytes: u64,
    /// `cost.rentBytes` from the node. Omit when the node did not report it —
    /// the quote will come back with a `null` refund rather than a zero one.
    #[serde(default)]
    pub rent_bytes: Option<u64>,
    /// Split of the written bytes by durability. Omit when unknown.
    #[serde(default)]
    pub temporary_write_bytes: Option<u64>,
    #[serde(default)]
    pub persistent_write_bytes: Option<u64>,
}

impl FeeQuoteRequest {
    fn durability_split(&self) -> DurabilitySplit {
        match (self.temporary_write_bytes, self.persistent_write_bytes) {
            (Some(temporary), Some(persistent)) => DurabilitySplit::mixed(temporary, persistent),
            // One half on its own is still a complete answer: the absent half is
            // a definite zero, not an unknown.
            (Some(temporary), None) => DurabilitySplit::temporary_only(temporary),
            (None, Some(persistent)) => DurabilitySplit::persistent_only(persistent),
            (None, None) => DurabilitySplit::unknown(),
        }
    }
}

/// `POST /fee/quote` — gross fee, estimated refund and estimated net.
pub async fn quote_fee(
    Json(request): Json<FeeQuoteRequest>,
) -> Result<Json<ResourceFeeQuote>, AppError> {
    let input = FeeQuoteInput {
        cpu_instructions: request.cpu_instructions,
        ledger_read_bytes: request.ledger_read_bytes,
        ledger_write_bytes: request.ledger_write_bytes,
        transaction_size_bytes: request.transaction_size_bytes,
        read_entries: request.read_entries,
        write_entries: request.write_entries,
        contract_event_bytes: request.contract_event_bytes,
        rent_bytes: request.rent_bytes,
    };
    let quote = ResourceFeeQuote::estimate(
        &input,
        request.durability_split(),
        SorobanFeeConfig::checked_in(),
    );
    Ok(Json(quote))
}

/// Request body for `POST /sac/transfer`.
///
/// Balances are supplied by the caller. When they are absent the transfer cannot
/// be priced at all (see [`crate::sac_transfer::SacError::BalanceEntryMissing`]),
/// so there is no "look it up for me" mode here on purpose: a fetch belongs in
/// whatever layer owns the node client, and it makes the result reproducible.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SacTransferApiRequest {
    #[serde(flatten)]
    pub transfer: SacTransferRequest,
    #[serde(default)]
    pub sender_balance: Option<i128>,
    #[serde(default)]
    pub destination_balance: Option<i128>,
}

/// `POST /sac/transfer` — profile a SAC `transfer`.
pub async fn simulate_sac(
    engine: &SimulationEngine,
    Json(payload): Json<SacTransferApiRequest>,
) -> Result<Json<SacTransferReport>, AppError> {
    let balances = SacBalances {
        sender: payload.sender_balance,
        destination: payload.destination_balance,
    };
    let report = sac_transfer::simulate_sac_transfer(
        engine,
        &payload.transfer,
        balances,
        // Injected by definition: the balances came in on the request.
        SacBalanceSource::Injected,
    )
    .await?;
    Ok(Json(report))
}

/// `POST /sac/transfer` as an Axum handler, for a router holding the engine in
/// its own state type.
pub fn sac_transfer_routes(engine: Arc<SimulationEngine>) -> Router {
    Router::new()
        .route("/sac/transfer", post(sac_transfer_with_engine))
        .with_state(engine)
}

async fn sac_transfer_with_engine(
    axum::extract::State(engine): axum::extract::State<Arc<SimulationEngine>>,
    payload: Json<SacTransferApiRequest>,
) -> Result<Json<SacTransferReport>, AppError> {
    simulate_sac(&engine, payload).await
}

/// Routes for the fee quote. No engine needed, so this router is stateless.
pub fn fee_quote_routes() -> Router {
    Router::new()
        .route("/fee/quote", post(quote_fee))
        .route("/fee/config", axum::routing::get(get_fee_config))
}

#[utoipa::path(
    get,
    path = "/fee/config",
    responses((status = 200, description = "Checked-in Soroban fee parameters", body = FeeConfigResponse)),
    tag = "Fees"
)]
async fn get_fee_config() -> Result<Json<FeeConfigResponse>, AppError> {
    Ok(Json(FeeConfigResponse {
        config: SorobanFeeConfig::checked_in().clone(),
    }))
}

/// State for the webhook dead-letter queue endpoints (#1399).
#[derive(Clone)]
pub struct WebhookDlqState {
    pub dlq: WebhookDlq,
    pub registry: SubscriptionRegistry,
    pub client: reqwest::Client,
    pub config: WebhookConfig,
}

/// `GET /api/v1/webhooks/dlq` and `POST /api/v1/webhooks/dlq/:id/replay`.
///
/// These expose failed deliveries and re-send them, so mount the router inside
/// the authenticated (`auth::auth_middleware`) part of the application.
pub fn webhook_dlq_routes(state: WebhookDlqState) -> Router {
    Router::new()
        .route("/api/v1/webhooks/dlq", axum::routing::get(list_webhook_dlq))
        .route("/api/v1/webhooks/dlq/:id/replay", post(replay_webhook_dlq))
        .with_state(state)
}

async fn list_webhook_dlq(
    axum::extract::State(state): axum::extract::State<WebhookDlqState>,
) -> Result<Json<Vec<DlqEntry>>, AppError> {
    state.dlq.list().map(Json).map_err(dlq_error)
}

async fn replay_webhook_dlq(
    axum::extract::State(state): axum::extract::State<WebhookDlqState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<ReplayOutcome>, AppError> {
    state
        .dlq
        .replay(id, &state.registry, &state.client, &state.config)
        .await
        .map(Json)
        .map_err(dlq_error)
}

fn dlq_error(error: WebhookError) -> AppError {
    match error {
        WebhookError::DlqEntryNotFound(_) | WebhookError::SubscriptionNotFound(_) => {
            AppError::NotFound(error.to_string())
        }
        other => AppError::Internal(other.to_string()),
    }
}

/// Quote a completed simulation, using the rent the node reported for it.
///
/// The result still comes back with a `null` refund when the durability split is
/// unknown — see [`crate::fee_quote`] — but rent no longer has to be threaded in
/// separately by the caller.
pub fn quote_for_simulation(simulation: &crate::simulation::SimulationResult) -> ResourceFeeQuote {
    fee_quote::quote_simulation(
        simulation,
        simulation.rent_bytes,
        SorobanFeeConfig::checked_in(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    async fn body_of(response: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body reads");
        serde_json::from_slice(&bytes).expect("json body")
    }

    fn dlq_state() -> WebhookDlqState {
        WebhookDlqState {
            dlq: WebhookDlq::in_memory().unwrap(),
            registry: SubscriptionRegistry::default(),
            client: reqwest::Client::new(),
            config: WebhookConfig::default(),
        }
    }

    #[tokio::test]
    async fn webhooks_dlq_endpoint_lists_failed_deliveries() {
        let state = dlq_state();
        let entry = DlqEntry {
            id: uuid::Uuid::new_v4(),
            subscription_id: uuid::Uuid::new_v4(),
            event_id: uuid::Uuid::new_v4(),
            target_url: "https://example.com/events".to_string(),
            payload: serde_json::json!({"event": "transfer"}),
            last_status: Some(503),
            error: "HTTP 503 Service Unavailable".to_string(),
            attempts: 5,
            failed_at: chrono::Utc::now(),
        };
        state.dlq.insert(&entry).unwrap();

        let response = webhook_dlq_routes(state)
            .oneshot(
                Request::get("/api/v1/webhooks/dlq")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_of(response).await;
        assert_eq!(json[0]["id"], entry.id.to_string());
        assert_eq!(json[0]["last_status"], 503);
        assert_eq!(json[0]["attempts"], 5);
    }

    #[tokio::test]
    async fn webhooks_dlq_replay_of_unknown_entry_is_not_found() {
        let response = webhook_dlq_routes(dlq_state())
            .oneshot(
                Request::post(format!(
                    "/api/v1/webhooks/dlq/{}/replay",
                    uuid::Uuid::new_v4()
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn durability_split_defaults_to_unknown() {
        let request = FeeQuoteRequest {
            cpu_instructions: 0,
            ledger_read_bytes: 0,
            ledger_write_bytes: 0,
            transaction_size_bytes: 0,
            read_entries: 0,
            write_entries: 0,
            contract_event_bytes: 0,
            rent_bytes: Some(10_240),
            temporary_write_bytes: None,
            persistent_write_bytes: None,
        };
        assert_eq!(request.durability_split(), DurabilitySplit::unknown());
    }

    #[test]
    fn one_sided_split_is_a_definite_not_an_unknown() {
        let mut request = FeeQuoteRequest {
            cpu_instructions: 0,
            ledger_read_bytes: 0,
            ledger_write_bytes: 0,
            transaction_size_bytes: 0,
            read_entries: 0,
            write_entries: 0,
            contract_event_bytes: 0,
            rent_bytes: Some(10_240),
            temporary_write_bytes: Some(10_240),
            persistent_write_bytes: None,
        };
        assert_eq!(
            request.durability_split(),
            DurabilitySplit::temporary_only(10_240)
        );

        request.temporary_write_bytes = None;
        request.persistent_write_bytes = Some(10_240);
        assert_eq!(
            request.durability_split(),
            DurabilitySplit::persistent_only(10_240)
        );
    }

    #[tokio::test]
    async fn quote_endpoint_reports_gross_and_null_refund_when_durability_is_omitted() {
        let app = fee_quote_routes();
        let response = app
            .oneshot(
                Request::post("/fee/quote")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"cpu_instructions":100000,"ledger_read_bytes":1024,
                            "ledger_write_bytes":1024,"transaction_size_bytes":1024,
                            "rent_bytes":10240}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_of(response).await;
        assert_eq!(json["refund_status"], "unknown_durability");
        assert!(json["estimated_refund"].is_null());
        assert!(json["estimated_net"].is_null());
        assert_eq!(json["gross_resource_fee"], 36_710);
        assert_eq!(json["refund_is_estimate"], true);
    }

    #[tokio::test]
    async fn quote_endpoint_reports_an_estimate_when_durability_is_given() {
        let app = fee_quote_routes();
        let response = app
            .oneshot(
                Request::post("/fee/quote")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"cpu_instructions":100000,"ledger_read_bytes":1024,
                            "ledger_write_bytes":1024,"transaction_size_bytes":1024,
                            "rent_bytes":10240,"temporary_write_bytes":10240,
                            "persistent_write_bytes":0}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_of(response).await;
        assert_eq!(json["refund_status"], "estimated");
        assert_eq!(json["estimated_refund"], 29);
        assert_eq!(json["estimated_net"], 36_652);
        assert_eq!(json["gross_resource_fee"], 36_681);
    }

    #[tokio::test]
    async fn fee_config_endpoint_exposes_the_checked_in_snapshot() {
        let app = fee_quote_routes();
        let response = app
            .oneshot(Request::get("/fee/config").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_of(response).await;
        assert_eq!(json["config"]["network"], "pubnet");
        assert_eq!(json["config"]["protocol"], 22);
        assert_eq!(json["config"]["temporary_rent_rate_denominator"], 4206);
    }

    #[tokio::test]
    async fn sac_endpoint_reports_a_missing_balance_as_a_client_error() {
        let engine = Arc::new(SimulationEngine::new("http://127.0.0.1:1".to_string()));
        let app = sac_transfer_routes(engine);

        let response = app
            .oneshot(
                Request::post("/sac/transfer")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"asset":{"kind":"native"},"from":"GAAQQDYWDUSCWMRZIBDU4VK4MNVHC6D7Q2GZJG5CVGYLPPWFZTJ5V6UJ",
                            "to":"GABASEAXDYSSYMZ2IFEE6VS5MRVXE6MAQ6HJLHFDVKY3RP6GZXKNWWW3",
                            "amount":1000,"network_passphrase":"Public Global Stellar Network ; September 2015",
                            "sender_balance":1000}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let json = body_of(response).await;
        // Errors are RFC 7807 problem details, so the human-readable text lives
        // in `detail`, not `message`.
        assert_eq!(json["status"], 400);
        assert_eq!(json["title"], "Bad Request");
        assert_eq!(json["type"], "https://soroscope.dev/errors/bad-request");
        let detail = json["detail"].as_str().unwrap();
        assert!(
            detail.contains("GABASEAXDYSSYMZ2IFEE6VS5MRVXE6MAQ6HJLHFDVKY3RP6GZXKNWWW3"),
            "the error must name the account: {json}"
        );
        assert!(detail.contains("destination"), "{json}");
    }

    #[tokio::test]
    async fn sac_endpoint_rejects_a_non_positive_amount() {
        let engine = Arc::new(SimulationEngine::new("http://127.0.0.1:1".to_string()));
        let app = sac_transfer_routes(engine);

        let response = app
            .oneshot(
                Request::post("/sac/transfer")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"asset":{"kind":"native"},"from":"GAAQQDYWDUSCWMRZIBDU4VK4MNVHC6D7Q2GZJG5CVGYLPPWFZTJ5V6UJ",
                            "to":"GABASEAXDYSSYMZ2IFEE6VS5MRVXE6MAQ6HJLHFDVKY3RP6GZXKNWWW3",
                            "amount":0,"network_passphrase":"Public Global Stellar Network ; September 2015",
                            "sender_balance":1000,"destination_balance":0}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let json = body_of(response).await;
        assert!(
            json["detail"].as_str().unwrap().contains("must be positive"),
            "{json}"
        );
    }
}
