//! Cross-Contract Call Trace Parser
//!
//! Parses Soroban host execution diagnostic events (base64-encoded XDR) to
//! build a nested `CallGraph` tree that captures cross-contract sub-calls
//! which are not visible in top-level event logs.
//!
//! The Soroban host emits two well-known diagnostic event types that mark
//! contract call boundaries:
//!
//! - `fn_call`   — emitted when a contract function is entered.
//!   Topics: `["fn_call", <contract_address>, <function_name>]`
//! - `fn_return` — emitted when a contract function exits.
//!   Topics: `["fn_return", <contract_address>, <function_name>]`
//!
//! This parser maintains a call stack: each `fn_call` pushes a new node and
//! each `fn_return` pops it, attaching it as a child of its caller.  The
//! final tree root represents the outermost contract invocation.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use soroban_sdk::xdr::{DiagnosticEvent, Hash, Limits, ReadXdr, ScVal};
use stellar_strkey::{Contract as StrkeyContract, Strkey};

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// A single node in the cross-contract call tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallNode {
    /// Stellar contract address (C…) or `"Host"` for host-native calls.
    pub contract_id: String,
    /// Name of the invoked function.
    pub function: String,
    /// Ordered list of nested sub-calls made during this invocation.
    pub children: Vec<CallNode>,
}

/// A complete call graph rooted at the outermost contract invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallGraph {
    pub root: CallNode,
}

impl CallGraph {
    /// Render the call tree as a [Mermaid](https://mermaid.js.org/) diagram.
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("graph TD\n");
        append_mermaid_nodes(&self.root, &mut mermaid, &mut 0);
        mermaid
    }

    /// Total number of nodes (calls) in the graph, including the root.
    pub fn total_calls(&self) -> usize {
        count_nodes(&self.root)
    }

    /// Maximum nesting depth of sub-calls (root = depth 1).
    pub fn max_depth(&self) -> usize {
        node_depth(&self.root)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parser
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a slice of base64-encoded XDR `DiagnosticEvent` strings and produce
/// a [`CallGraph`] if at least one complete `fn_call`/`fn_return` pair is
/// found.
///
/// Events that cannot be decoded or do not match the expected schema are
/// silently skipped so that partial or future-format event streams degrade
/// gracefully.
pub fn parse_call_trace(events: &[String]) -> Option<CallGraph> {
    let mut stack: Vec<CallNode> = Vec::new();
    let mut root: Option<CallNode> = None;

    for event_b64 in events {
        let bytes = match BASE64.decode(event_b64) {
            Ok(b) => b,
            Err(_) => continue,
        };

        let diag_event = match DiagnosticEvent::from_xdr(&bytes, Limits::none()) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Only process events from successful contract calls.
        if !diag_event.in_successful_contract_call {
            continue;
        }

        let contract_id = match &diag_event.event.contract_id {
            Some(Hash(h)) => Strkey::Contract(StrkeyContract(*h)).to_string(),
            None => "Host".to_string(),
        };

        let topics = match &diag_event.event.body {
            soroban_sdk::xdr::ContractEventBody::V0(v0) => &v0.topics,
        };

        if topics.is_empty() {
            continue;
        }

        let topic0 = match &topics[0] {
            ScVal::Symbol(s) => s.to_string(),
            _ => continue,
        };

        match topic0.as_str() {
            "fn_call" if topics.len() >= 3 => {
                let function = match &topics[2] {
                    ScVal::Symbol(s) => s.to_string(),
                    _ => "unknown".to_string(),
                };
                stack.push(CallNode {
                    contract_id,
                    function,
                    children: Vec::new(),
                });
            }
            "fn_return" => {
                if let Some(finished_node) = stack.pop() {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(finished_node);
                    } else {
                        root = Some(finished_node);
                    }
                }
            }
            _ => {}
        }
    }

    // Flush any dangling stack entries as top-level roots (malformed traces).
    // If there is exactly one remaining node and no root yet, use it.
    if root.is_none() {
        if let Some(node) = stack.into_iter().next() {
            root = Some(node);
        }
    }

    root.map(|r| CallGraph { root: r })
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn append_mermaid_nodes(node: &CallNode, mermaid: &mut String, id_gen: &mut usize) {
    let current_id = *id_gen;
    mermaid.push_str(&format!(
        "    n{current_id}[\"{}\\n{}\"]\n",
        node.contract_id, node.function
    ));
    for child in &node.children {
        *id_gen += 1;
        let child_id = *id_gen;
        mermaid.push_str(&format!("    n{current_id} --> n{child_id}\n"));
        append_mermaid_nodes(child, mermaid, id_gen);
    }
}

fn count_nodes(node: &CallNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

fn node_depth(node: &CallNode) -> usize {
    if node.children.is_empty() {
        1
    } else {
        1 + node.children.iter().map(node_depth).max().unwrap_or(0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diag_event(topic0: &str, fn_name: &str) -> String {
        use soroban_sdk::xdr::{
            ContractEvent, ContractEventBody, ContractEventType, ContractEventV0, DiagnosticEvent,
            ScSymbol, ScVal, StringM, VecM, WriteXdr,
        };

        let sym = |s: &str| -> ScVal {
            let string_m: StringM = s.as_bytes().to_vec().try_into().unwrap();
            ScVal::Symbol(ScSymbol(string_m))
        };

        let topics: VecM<ScVal> = vec![sym(topic0), sym("CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM"), sym(fn_name)]
            .try_into()
            .unwrap();

        let event = DiagnosticEvent {
            in_successful_contract_call: true,
            event: ContractEvent {
                ext: soroban_sdk::xdr::ExtensionPoint::V0,
                contract_id: None,
                type_: ContractEventType::Diagnostic,
                body: ContractEventBody::V0(ContractEventV0 {
                    topics,
                    data: ScVal::Void,
                }),
            },
        };

        let bytes = event.to_xdr(Limits::none()).unwrap();
        BASE64.encode(bytes)
    }

    #[test]
    fn test_empty_events_returns_none() {
        assert_eq!(parse_call_trace(&[]), None);
    }

    #[test]
    fn test_single_call_return_pair() {
        let events = vec![
            make_diag_event("fn_call", "transfer"),
            make_diag_event("fn_return", "transfer"),
        ];
        let graph = parse_call_trace(&events).expect("should produce a graph");
        assert_eq!(graph.root.function, "transfer");
        assert!(graph.root.children.is_empty());
        assert_eq!(graph.total_calls(), 1);
        assert_eq!(graph.max_depth(), 1);
    }

    #[test]
    fn test_nested_sub_call() {
        // outer calls inner
        let events = vec![
            make_diag_event("fn_call", "outer"),
            make_diag_event("fn_call", "inner"),
            make_diag_event("fn_return", "inner"),
            make_diag_event("fn_return", "outer"),
        ];
        let graph = parse_call_trace(&events).expect("should produce a graph");
        assert_eq!(graph.root.function, "outer");
        assert_eq!(graph.root.children.len(), 1);
        assert_eq!(graph.root.children[0].function, "inner");
        assert_eq!(graph.total_calls(), 2);
        assert_eq!(graph.max_depth(), 2);
    }

    #[test]
    fn test_mermaid_output_contains_nodes() {
        let events = vec![
            make_diag_event("fn_call", "mint"),
            make_diag_event("fn_return", "mint"),
        ];
        let graph = parse_call_trace(&events).unwrap();
        let mermaid = graph.to_mermaid();
        assert!(mermaid.starts_with("graph TD\n"));
        assert!(mermaid.contains("mint"));
    }

    #[test]
    fn test_invalid_base64_skipped() {
        let events = vec![
            "not-valid-base64!!".to_string(),
            make_diag_event("fn_call", "safe"),
            make_diag_event("fn_return", "safe"),
        ];
        let graph = parse_call_trace(&events).expect("should still produce a graph");
        assert_eq!(graph.root.function, "safe");
    }

    #[test]
    fn test_dangling_call_without_return() {
        // Only a fn_call with no fn_return — should still produce a graph.
        let events = vec![make_diag_event("fn_call", "dangling")];
        let graph = parse_call_trace(&events).expect("should produce a graph for dangling call");
        assert_eq!(graph.root.function, "dangling");
    }
}
