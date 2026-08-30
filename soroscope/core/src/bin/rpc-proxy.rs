use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value = "8545")]
    port: u16,
    #[arg(short, long)]
    rpc_url: String,
}

#[derive(Clone)]
#[allow(dead_code)]
struct Config {
    max_gas_limit: u64,
    min_gas_price: u64,
    blocked_addresses: HashSet<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_gas_limit: 30_000_000,
            min_gas_price: 100,
            blocked_addresses: HashSet::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct RpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
    id: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Transaction {
    from: String,
    to: Option<String>,
    gas: Option<String>,
    value: Option<String>,
    data: Option<String>,
}

struct AppState {
    rpc_url: String,
    client: reqwest::Client,
    config: Config,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let state = Arc::new(AppState {
        rpc_url: args.rpc_url,
        client: reqwest::Client::new(),
        config: Config::default(),
    });

    let app = Router::new().route("/", post(handle_rpc)).with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port))
        .await
        .unwrap();

    println!("RPC Proxy running on port {}", args.port);
    axum::serve(listener, app).await.unwrap();
}

async fn handle_rpc(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RpcRequest>,
) -> impl IntoResponse {
    if req.method == "eth_sendTransaction" {
        println!("Intercepting sendTransaction");

        let params: Vec<Transaction> = match serde_json::from_value(req.params.clone()) {
            Ok(p) => p,
            Err(_) => {
                return Json(RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(RpcError {
                        code: -32602,
                        message: "Invalid params".to_string(),
                    }),
                    id: req.id,
                });
            }
        };

        if params.is_empty() {
            return Json(RpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(RpcError {
                    code: -32602,
                    message: "Missing transaction params".to_string(),
                }),
                id: req.id,
            });
        }

        let tx = &params[0];

        if state.config.blocked_addresses.contains(&tx.from) {
            return Json(RpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(RpcError {
                    code: -32000,
                    message: "Address is blocked".to_string(),
                }),
                id: req.id,
            });
        }

        let simulate_req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [tx, "latest"],
            "id": 1
        });

        match state
            .client
            .post(&state.rpc_url)
            .json(&simulate_req)
            .send()
            .await
        {
            Ok(resp) => {
                let result: serde_json::Value = resp.json().await.unwrap_or_default();

                if result.get("error").is_some() {
                    println!("Simulation failed for tx from {}", tx.from);
                    return Json(RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(RpcError {
                            code: -32000,
                            message: "Transaction would fail".to_string(),
                        }),
                        id: req.id,
                    });
                }

                let gas_req = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "eth_estimateGas",
                    "params": [tx],
                    "id": 1
                });

                if let Ok(gas_resp) = state
                    .client
                    .post(&state.rpc_url)
                    .json(&gas_req)
                    .send()
                    .await
                {
                    let gas_body: serde_json::Value = gas_resp.json().await.unwrap_or_default();
                    let gas_used = gas_body
                        .get("result")
                        .and_then(|r| r.as_str())
                        .and_then(|s| u64::from_str_radix(&s[2..], 16).ok())
                        .unwrap_or(0);

                    if gas_used > state.config.max_gas_limit {
                        println!(
                            "Gas limit exceeded: {} > {}",
                            gas_used, state.config.max_gas_limit
                        );
                        return Json(RpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(RpcError {
                                code: -32000,
                                message: format!(
                                    "Gas limit exceeded: {} > {}",
                                    gas_used, state.config.max_gas_limit
                                ),
                            }),
                            id: req.id,
                        });
                    }

                    println!("Simulation passed: gas={}", gas_used);
                }
            }
            Err(_) => {
                return Json(RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(RpcError {
                        code: -32000,
                        message: "Simulation failed".to_string(),
                    }),
                    id: req.id,
                });
            }
        }
    }

    match state.client.post(&state.rpc_url).json(&req).send().await {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            Json(RpcResponse {
                jsonrpc: "2.0".to_string(),
                result: body.get("result").cloned(),
                error: None,
                id: req.id,
            })
        }
        Err(e) => Json(RpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(RpcError {
                code: -32000,
                message: format!("Upstream error: {}", e),
            }),
            id: req.id,
        }),
    }
}
