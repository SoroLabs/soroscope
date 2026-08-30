//! GraphQL query layer for contract execution history and token metadata.
//!
//! REST clients previously needed multiple round-trips to assemble a
//! contract's execution history (one `/jobs/{id}` call per job) or a
//! token's SEP-41 metadata (one `/analyze` call per `name`/`symbol`/
//! `decimals` function). This module exposes both as a single paginated,
//! filterable GraphQL query via `async-graphql` mounted on the existing
//! Axum router.

use async_graphql::{
    Context, EmptyMutation, EmptySubscription, Enum, Object, Schema, SimpleObject,
};

use crate::jobs::{Job, JobListFilter, JobPayload, JobQueue, JobResult};
use crate::jobs::{JobStatus as CoreJobStatus, JobType as CoreJobType};
use crate::simulation::{SimulationEngine, SorobanResources};

/// The soroscope-core GraphQL schema: query-only (no mutations/subscriptions).
pub type SoroscopeSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

/// Build the schema, wiring in the data sources resolvers read from.
pub fn build_schema(job_queue: JobQueue, engine: SimulationEngine) -> SoroscopeSchema {
    Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(job_queue)
        .data(engine)
        .finish()
}

/// Mirrors [`crate::jobs::JobStatus`] as a GraphQL-facing filter enum.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum JobStatusFilter {
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl From<JobStatusFilter> for CoreJobStatus {
    fn from(value: JobStatusFilter) -> Self {
        match value {
            JobStatusFilter::Queued => CoreJobStatus::Queued,
            JobStatusFilter::Processing => CoreJobStatus::Processing,
            JobStatusFilter::Completed => CoreJobStatus::Completed,
            JobStatusFilter::Failed => CoreJobStatus::Failed,
            JobStatusFilter::Cancelled => CoreJobStatus::Cancelled,
        }
    }
}

/// Mirrors [`crate::jobs::JobType`] as a GraphQL-facing filter enum.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum JobTypeFilter {
    Analyze,
    Compare,
    OptimizeLimits,
}

impl From<JobTypeFilter> for CoreJobType {
    fn from(value: JobTypeFilter) -> Self {
        match value {
            JobTypeFilter::Analyze => CoreJobType::Analyze,
            JobTypeFilter::Compare => CoreJobType::Compare,
            JobTypeFilter::OptimizeLimits => CoreJobType::OptimizeLimits,
        }
    }
}

/// Resource metrics for a completed execution. Large counters are rendered
/// as strings to avoid precision loss in GraphQL's `Float`/`Int` scalars.
#[derive(SimpleObject, Clone)]
pub struct ExecutionResources {
    pub cpu_instructions: String,
    pub ram_bytes: String,
    pub ledger_read_bytes: String,
    pub ledger_write_bytes: String,
    pub transaction_size_bytes: String,
}

impl From<&SorobanResources> for ExecutionResources {
    fn from(resources: &SorobanResources) -> Self {
        Self {
            cpu_instructions: resources.cpu_instructions.to_string(),
            ram_bytes: resources.ram_bytes.to_string(),
            ledger_read_bytes: resources.ledger_read_bytes.to_string(),
            ledger_write_bytes: resources.ledger_write_bytes.to_string(),
            transaction_size_bytes: resources.transaction_size_bytes.to_string(),
        }
    }
}

/// One entry in a contract's execution history.
#[derive(SimpleObject, Clone)]
pub struct ContractExecution {
    pub id: String,
    pub job_type: String,
    pub status: String,
    pub contract_id: Option<String>,
    pub function_name: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub resources: Option<ExecutionResources>,
    pub error: Option<String>,
}

impl From<&Job> for ContractExecution {
    fn from(job: &Job) -> Self {
        let payload = job.get_payload();
        let contract_id = payload
            .as_ref()
            .and_then(JobPayload::contract_id)
            .map(str::to_string);
        let function_name = payload
            .as_ref()
            .and_then(JobPayload::function_name)
            .map(str::to_string);

        let (resources, error) = match job.get_result() {
            Some(JobResult::Success {
                resources: Some(resources),
                ..
            }) => (Some(ExecutionResources::from(&resources)), None),
            Some(JobResult::Success { .. }) => (None, None),
            Some(JobResult::Failed { error, .. }) => (None, Some(error)),
            None => (None, None),
        };

        Self {
            id: job.id.to_string(),
            job_type: format!("{:?}", job.job_type),
            status: format!("{:?}", job.status),
            contract_id,
            function_name,
            created_at: job.created_at.to_rfc3339(),
            completed_at: job.completed_at.map(|ts| ts.to_rfc3339()),
            resources,
            error,
        }
    }
}

/// SEP-41 token metadata assembled from three simulated invocations.
#[derive(SimpleObject, Clone)]
pub struct TokenMetadata {
    pub contract_id: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<i32>,
}

/// Best-effort decode of a token metadata function's XDR return value into
/// a display string (`ScVal::String`/`Symbol`/`Bytes` all render as text).
fn scval_to_display_string(value: &soroban_sdk::xdr::ScVal) -> Option<String> {
    use soroban_sdk::xdr::ScVal;
    match value {
        ScVal::String(s) => Some(s.to_string()),
        ScVal::Symbol(s) => Some(s.to_string()),
        ScVal::Bytes(b) => String::from_utf8(b.to_vec()).ok(),
        _ => None,
    }
}

fn scval_to_u32(value: &soroban_sdk::xdr::ScVal) -> Option<i32> {
    use soroban_sdk::xdr::ScVal;
    match value {
        ScVal::U32(n) => Some(*n as i32),
        ScVal::I32(n) => Some(*n),
        _ => None,
    }
}

async fn fetch_token_metadata(engine: &SimulationEngine, contract_id: &str) -> TokenMetadata {
    let (name_result, symbol_result, decimals_result) = tokio::join!(
        engine.invoke_read_only(contract_id, "name"),
        engine.invoke_read_only(contract_id, "symbol"),
        engine.invoke_read_only(contract_id, "decimals"),
    );

    let name = name_result
        .ok()
        .flatten()
        .and_then(|v| scval_to_display_string(&v));
    let symbol = symbol_result
        .ok()
        .flatten()
        .and_then(|v| scval_to_display_string(&v));
    let decimals = decimals_result
        .ok()
        .flatten()
        .and_then(|v| scval_to_u32(&v));

    TokenMetadata {
        contract_id: contract_id.to_string(),
        name,
        symbol,
        decimals,
    }
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Contract execution history, most-recent-first, with optional
    /// filtering and offset/limit pagination.
    async fn contract_executions(
        &self,
        ctx: &Context<'_>,
        contract_id: Option<String>,
        status: Option<JobStatusFilter>,
        job_type: Option<JobTypeFilter>,
        #[graphql(default = 20)] limit: i32,
        #[graphql(default = 0)] offset: i32,
    ) -> async_graphql::Result<Vec<ContractExecution>> {
        let job_queue = ctx.data::<JobQueue>()?;
        let filter = JobListFilter {
            status: status.map(Into::into),
            job_type: job_type.map(Into::into),
            contract_id,
        };

        let jobs = job_queue
            .list(&filter, limit as i64, offset as i64)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(jobs.iter().map(ContractExecution::from).collect())
    }

    /// SEP-41 token metadata (name/symbol/decimals) for a deployed contract,
    /// assembled from three simulated invocations in a single query.
    async fn token_metadata(
        &self,
        ctx: &Context<'_>,
        contract_id: String,
    ) -> async_graphql::Result<TokenMetadata> {
        let engine = ctx.data::<SimulationEngine>()?;
        Ok(fetch_token_metadata(engine, &contract_id).await)
    }
}

// A dedicated `async-graphql-axum` integration isn't used here: its 7.x
// releases require axum 0.8, while this crate is pinned to axum 0.7. The
// handler below talks to `async-graphql` directly instead — a GraphQL
// request/response is just JSON, so this is a thin, dependency-free bridge.

/// `POST /graphql` — executes a GraphQL request against [`SoroscopeSchema`].
pub async fn graphql_handler(
    axum::extract::Extension(schema): axum::extract::Extension<SoroscopeSchema>,
    axum::Json(request): axum::Json<async_graphql::Request>,
) -> axum::Json<async_graphql::Response> {
    axum::Json(schema.execute(request).await)
}

/// `GET /graphql` — serves the GraphiQL IDE for interactive exploration.
pub async fn graphql_playground() -> impl axum::response::IntoResponse {
    axum::response::Html(
        async_graphql::http::GraphiQLSource::build()
            .endpoint("/graphql")
            .finish(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{JobId, JobStatus, JobType};
    use chrono::Utc;
    use soroban_sdk::xdr::{ScString, ScSymbol, ScVal};

    fn base_job(payload: JobPayload, result: Option<JobResult>) -> Job {
        Job {
            id: JobId::new(),
            job_type: JobType::Analyze,
            status: JobStatus::Completed,
            payload: serde_json::to_value(&payload).unwrap(),
            result: result.map(|r| serde_json::to_value(&r).unwrap()),
            progress_percent: 100,
            progress_message: "Completed".to_string(),
            webhook_url: None,
            webhook_headers: None,
            webhook_secret: None,
            error_message: None,
            error_type: None,
            timeout_secs: 300,
            retry_count: 0,
            created_at: Utc::now(),
            started_at: None,
            completed_at: Some(Utc::now()),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn contract_execution_extracts_contract_id_and_resources_from_a_successful_job() {
        let payload = JobPayload::Analyze {
            contract_id: "CABC".to_string(),
            function_name: "hello".to_string(),
            args: None,
            ledger_overrides: None,
        };
        let result = JobResult::Success {
            resources: Some(SorobanResources {
                cpu_instructions: 1_000,
                ram_bytes: 2_000,
                ledger_read_bytes: 3_000,
                ledger_write_bytes: 4_000,
                transaction_size_bytes: 5_000,
            }),
            simulation_result: None,
            optimization: None,
            comparison: None,
        };
        let job = base_job(payload, Some(result));

        let execution = ContractExecution::from(&job);
        assert_eq!(execution.contract_id, Some("CABC".to_string()));
        assert_eq!(execution.function_name, Some("hello".to_string()));
        assert!(execution.error.is_none());
        let resources = execution.resources.expect("resources present");
        assert_eq!(resources.cpu_instructions, "1000");
        assert_eq!(resources.ram_bytes, "2000");
    }

    #[test]
    fn contract_execution_surfaces_the_error_for_a_failed_job() {
        let payload = JobPayload::Analyze {
            contract_id: "CABC".to_string(),
            function_name: "hello".to_string(),
            args: None,
            ledger_overrides: None,
        };
        let result = JobResult::Failed {
            error: "simulation timed out".to_string(),
            error_type: "Timeout".to_string(),
        };
        let job = base_job(payload, Some(result));

        let execution = ContractExecution::from(&job);
        assert_eq!(execution.error, Some("simulation timed out".to_string()));
        assert!(execution.resources.is_none());
    }

    #[test]
    fn contract_execution_handles_a_job_with_no_result_yet() {
        let payload = JobPayload::Analyze {
            contract_id: "CABC".to_string(),
            function_name: "hello".to_string(),
            args: None,
            ledger_overrides: None,
        };
        let job = base_job(payload, None);

        let execution = ContractExecution::from(&job);
        assert!(execution.resources.is_none());
        assert!(execution.error.is_none());
    }

    #[test]
    fn scval_to_display_string_decodes_string_symbol_and_bytes() {
        let s = ScString(
            "USD Coin"
                .as_bytes()
                .to_vec()
                .try_into()
                .expect("valid string"),
        );
        assert_eq!(
            scval_to_display_string(&ScVal::String(s)),
            Some("USD Coin".to_string())
        );

        let sym: ScSymbol = "USDC".try_into().unwrap();
        assert_eq!(
            scval_to_display_string(&ScVal::Symbol(sym)),
            Some("USDC".to_string())
        );

        assert_eq!(scval_to_display_string(&ScVal::Void), None);
    }

    #[test]
    fn scval_to_u32_decodes_u32_and_i32() {
        assert_eq!(scval_to_u32(&ScVal::U32(7)), Some(7));
        assert_eq!(scval_to_u32(&ScVal::I32(-1)), Some(-1));
        assert_eq!(scval_to_u32(&ScVal::Void), None);
    }

    #[test]
    fn job_status_and_type_filters_convert_to_core_enums() {
        assert_eq!(
            CoreJobStatus::from(JobStatusFilter::Completed),
            CoreJobStatus::Completed
        );
        assert_eq!(
            CoreJobStatus::from(JobStatusFilter::Failed),
            CoreJobStatus::Failed
        );
        assert_eq!(
            CoreJobType::from(JobTypeFilter::OptimizeLimits),
            CoreJobType::OptimizeLimits
        );
    }
}
