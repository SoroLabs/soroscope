//! Decodes Soroban transaction-result XDR into API-friendly diagnostics.

use base64::prelude::BASE64_STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use soroban_sdk::xdr::{
    ContractEvent, ContractEventBody, HostFunction, Limits, Operation, OperationBody, ReadXdr,
    SorobanTransactionMeta, SorobanTransactionMetaExt, TransactionEnvelope, TransactionMeta,
    TransactionResultMeta,
};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecodedTransactionResult {
    pub invocations: Vec<DecodedInvocation>,
    pub events: Vec<DecodedContractEvent>,
    pub diagnostics: Vec<DecodedDiagnosticEvent>,
    pub return_value: String,
    pub fees: ResourceFeeBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecodedInvocation {
    /// Transaction source account in lossless XDR debug representation.
    pub invoker: String,
    pub contract: String,
    pub function: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecodedContractEvent {
    pub contract_id: Option<String>,
    pub event_type: String,
    pub topics: Vec<String>,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecodedDiagnosticEvent {
    pub in_successful_contract_call: bool,
    pub event: DecodedContractEvent,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceFeeBreakdown {
    pub non_refundable: i64,
    pub refundable: i64,
    pub rent: i64,
    pub total: i64,
}

/// Detailed error information containing the failure kind, byte offset, and message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Error)]
#[error("invalid {kind} XDR{}: {message}", match .offset { Some(off) => format!(" at byte offset {off}"), None => String::new() })]
pub struct XdrDecodeErrorInfo {
    pub kind: String,
    pub offset: Option<usize>,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum XdrDecodeError {
    #[error("invalid {kind} XDR{}: {message}", match .offset { Some(off) => format!(" at byte offset {off}"), None => String::new() })]
    InvalidXdr {
        kind: &'static str,
        offset: Option<usize>,
        message: String,
        #[source]
        source: Option<soroban_sdk::xdr::Error>,
    },
    #[error("invalid base64 encoding{}: {message}", match .offset { Some(off) => format!(" at byte offset {off}"), None => String::new() })]
    InvalidBase64 {
        offset: Option<usize>,
        message: String,
    },
    #[error("corrupted XDR payload: {0}")]
    CorruptPayload(XdrDecodeErrorInfo),
    #[error("transaction metadata does not contain Soroban execution data")]
    MissingSorobanMetadata,
}

impl XdrDecodeError {
    pub fn offset(&self) -> Option<usize> {
        match self {
            XdrDecodeError::InvalidXdr { offset, .. } => *offset,
            XdrDecodeError::InvalidBase64 { offset, .. } => *offset,
            XdrDecodeError::CorruptPayload(info) => info.offset,
            XdrDecodeError::MissingSorobanMetadata => None,
        }
    }

    pub fn to_error_info(&self) -> XdrDecodeErrorInfo {
        match self {
            XdrDecodeError::InvalidXdr {
                kind,
                offset,
                message,
                ..
            } => XdrDecodeErrorInfo {
                kind: kind.to_string(),
                offset: *offset,
                message: message.clone(),
            },
            XdrDecodeError::InvalidBase64 { offset, message } => XdrDecodeErrorInfo {
                kind: "base64".to_string(),
                offset: *offset,
                message: message.clone(),
            },
            XdrDecodeError::CorruptPayload(info) => info.clone(),
            XdrDecodeError::MissingSorobanMetadata => XdrDecodeErrorInfo {
                kind: "soroban_metadata".to_string(),
                offset: None,
                message: "transaction metadata does not contain Soroban execution data".to_string(),
            },
        }
    }
}

pub struct XdrTransactionResultDecoder;

impl XdrTransactionResultDecoder {
    /// Helper to decode base64 into raw bytes and then parse XDR with byte offset tracking.
    #[allow(dead_code)]
    fn decode_xdr_base64<T: ReadXdr>(xdr: &str, kind: &'static str) -> Result<T, XdrDecodeError> {
        let trimmed = xdr.trim();
        if trimmed.is_empty() {
            return Err(XdrDecodeError::InvalidBase64 {
                offset: None,
                message: "empty base64 string".to_string(),
            });
        }
        let bytes = match BASE64.decode(trimmed.as_bytes()) {
            Ok(b) => b,
            Err(e) => {
                let offset = match e {
                    base64::DecodeError::InvalidByte(offset, _) => Some(offset),
                    base64::DecodeError::InvalidLength(len) => Some(len),
                    base64::DecodeError::InvalidLastSymbol(offset, _) => Some(offset),
                    base64::DecodeError::InvalidPadding => None,
                };
                return Err(XdrDecodeError::InvalidBase64 {
                    offset,
                    message: format!("base64 decode error: {e}"),
                });
            }
        };

        match T::from_xdr(&bytes, Limits::none()) {
            Ok(val) => Ok(val),
            Err(source) => Err(XdrDecodeError::InvalidXdr {
                kind,
                offset: Some(0),
                message: format!("{source}"),
                source: Some(source),
            }),
        }
    }

    /// Decodes the `resultMetaXdr` returned by Soroban RPC `getTransaction`.
    pub fn decode_result_meta(xdr: &str) -> Result<DecodedTransactionResult, XdrDecodeError> {
        let result: TransactionResultMeta = Self::decode_xdr_base64(xdr, "transaction result metadata")?;
        let meta = soroban_meta(&result.tx_apply_processing)
            .ok_or(XdrDecodeError::MissingSorobanMetadata)?;
        Ok(Self::decode_soroban_meta(meta))
    }

    /// Decodes standalone `SorobanTransactionMeta` XDR.
    pub fn decode_soroban_meta_xdr(xdr: &str) -> Result<DecodedTransactionResult, XdrDecodeError> {
        let meta: SorobanTransactionMeta = Self::decode_xdr_base64(xdr, "Soroban transaction metadata")?;
        Ok(Self::decode_soroban_meta(&meta))
    }

    /// Decodes host-function invocations from an RPC `envelopeXdr` value.
    pub fn decode_envelope(xdr: &str) -> Result<Vec<DecodedInvocation>, XdrDecodeError> {
        let envelope: TransactionEnvelope = Self::decode_xdr_base64(xdr, "transaction envelope")?;
        Ok(decode_envelope(&envelope))
    }

    /// Decodes a result and optionally enriches it with invocation details.
    pub fn decode(
        result_xdr: &str,
        envelope_xdr: Option<&str>,
    ) -> Result<DecodedTransactionResult, XdrDecodeError> {
        let mut decoded = Self::decode_result_meta(result_xdr)?;
        if let Some(xdr) = envelope_xdr {
            decoded.invocations = Self::decode_envelope(xdr)?;
        }
        Ok(decoded)
    }

    pub fn decode_soroban_meta(meta: &SorobanTransactionMeta) -> DecodedTransactionResult {
        let fees = match &meta.ext {
            SorobanTransactionMetaExt::V0 => ResourceFeeBreakdown::default(),
            SorobanTransactionMetaExt::V1(fees) => ResourceFeeBreakdown {
                non_refundable: fees.total_non_refundable_resource_fee_charged,
                refundable: fees.total_refundable_resource_fee_charged,
                rent: fees.rent_fee_charged,
                total: fees.total_non_refundable_resource_fee_charged
                    + fees.total_refundable_resource_fee_charged,
            },
        };
        DecodedTransactionResult {
            invocations: Vec::new(),
            events: meta.events.iter().map(decode_event).collect(),
            diagnostics: meta
                .diagnostic_events
                .iter()
                .map(|diagnostic| DecodedDiagnosticEvent {
                    in_successful_contract_call: diagnostic.in_successful_contract_call,
                    event: decode_event(&diagnostic.event),
                })
                .collect(),
            return_value: format_sc_val(&meta.return_value),
            fees,
        }
    }
}

fn soroban_meta(meta: &TransactionMeta) -> Option<&SorobanTransactionMeta> {
    match meta {
        TransactionMeta::V3(meta) => meta.soroban_meta.as_ref(),
        TransactionMeta::V0(_) | TransactionMeta::V1(_) | TransactionMeta::V2(_) => None,
    }
}

fn decode_envelope(envelope: &TransactionEnvelope) -> Vec<DecodedInvocation> {
    match envelope {
        TransactionEnvelope::Tx(envelope) => {
            decode_operations(&envelope.tx.source_account, &envelope.tx.operations)
        }
        TransactionEnvelope::TxFeeBump(envelope) => match &envelope.tx.inner_tx {
            soroban_sdk::xdr::FeeBumpTransactionInnerTx::Tx(inner) => {
                decode_operations(&inner.tx.source_account, &inner.tx.operations)
            }
        },
        TransactionEnvelope::TxV0(_) => Vec::new(),
    }
}

fn decode_operations(
    source: &soroban_sdk::xdr::MuxedAccount,
    operations: &[Operation],
) -> Vec<DecodedInvocation> {
    operations
        .iter()
        .filter_map(|operation| match &operation.body {
            OperationBody::InvokeHostFunction(op) => match &op.host_function {
                HostFunction::InvokeContract(args) => Some(DecodedInvocation {
                    invoker: format!("{source:?}"),
                    contract: format!("{:?}", args.contract_address),
                    function: format_symbol(&args.function_name),
                    arguments: args.args.iter().map(format_sc_val).collect(),
                }),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn decode_event(event: &ContractEvent) -> DecodedContractEvent {
    let (topics, data) = match &event.body {
        ContractEventBody::V0(body) => (
            body.topics.iter().map(format_sc_val).collect(),
            format_sc_val(&body.data),
        ),
    };
    DecodedContractEvent {
        contract_id: event.contract_id.as_ref().map(|id| hex::encode(id.0)),
        event_type: event.type_.to_string(),
        topics,
        data,
    }
}

fn format_symbol(symbol: &soroban_sdk::xdr::ScSymbol) -> String {
    String::from_utf8_lossy(symbol.0.as_ref()).into_owned()
}

fn format_sc_val(value: &soroban_sdk::xdr::ScVal) -> String {
    use soroban_sdk::xdr::ScVal;
    match value {
        ScVal::Void => "void".to_string(),
        ScVal::Bool(value) => value.to_string(),
        ScVal::U32(value) => value.to_string(),
        ScVal::I32(value) => value.to_string(),
        ScVal::U64(value) => value.to_string(),
        ScVal::I64(value) => value.to_string(),
        ScVal::U128(value) => format!("{value:?}"),
        ScVal::I128(value) => format!("{value:?}"),
        ScVal::U256(value) => format!("{value:?}"),
        ScVal::I256(value) => format!("{value:?}"),
        ScVal::Symbol(value) => format!(":{}", format_symbol(value)),
        ScVal::String(value) => String::from_utf8_lossy(value.0.as_ref()).into_owned(),
        ScVal::Bytes(value) => format!("0x{}", hex::encode(value.0.as_slice())),
        _ => format!("{value:?}"),
    }
}

// ── Soroban Bytecode Disassembler Utility (Issue #8) ──────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisassemblyInstruction {
    pub offset: usize,
    pub op: String,
    pub args: String,
    pub estimated_gas_cost: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisassembledModule {
    pub wat: String,
    pub instructions: Vec<DisassemblyInstruction>,
    pub function_count: usize,
    pub import_count: usize,
    pub export_count: usize,
    pub total_gas_metric: u64,
}

pub struct SorobanDisassembler;

impl SorobanDisassembler {
    /// Disassemble raw Soroban WASM bytecode into structured disassembly & WAT.
    pub fn disassemble_bytes(bytes: &[u8]) -> Result<DisassembledModule, XdrDecodeError> {
        let wat = wasmprinter::print_bytes(bytes).map_err(|e| XdrDecodeError::InvalidXdr {
            kind: "WASM bytecode disassembly",
            offset: None,
            message: format!("wasmprinter error: {e}"),
            source: None,
        })?;

        let mut instructions = Vec::new();
        let mut function_count = 0;
        let mut import_count = 0;
        let mut export_count = 0;
        let mut total_gas_metric = 0u64;

        let parser = wasmparser::Parser::new(0);
        for payload in parser.parse_all(bytes) {
            let payload = match payload {
                Ok(p) => p,
                Err(e) => {
                    return Err(XdrDecodeError::InvalidXdr {
                        kind: "WASM binary structure",
                        offset: Some(e.offset()),
                        message: format!("wasmparser error: {e}"),
                        source: None,
                    });
                }
            };

            match payload {
                wasmparser::Payload::FunctionSection(reader) => {
                    function_count = reader.count() as usize;
                }
                wasmparser::Payload::ImportSection(reader) => {
                    import_count = reader.count() as usize;
                }
                wasmparser::Payload::ExportSection(reader) => {
                    export_count = reader.count() as usize;
                }
                wasmparser::Payload::CodeSectionEntry(body) => {
                    if let Ok(mut ops_reader) = body.get_operators_reader() {
                        while !ops_reader.eof() {
                            let offset = ops_reader.original_position();
                            if let Ok(op) = ops_reader.read() {
                                let op_str = format!("{op:?}");
                                let (op_name, args) = match op_str.split_once(' ') {
                                    Some((n, a)) => (n.to_string(), a.to_string()),
                                    None => (op_str.clone(), String::new()),
                                };
                                let gas_cost = estimate_op_gas(&op_name);
                                total_gas_metric += gas_cost;
                                instructions.push(DisassemblyInstruction {
                                    offset,
                                    op: op_name,
                                    args,
                                    estimated_gas_cost: gas_cost,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(DisassembledModule {
            wat,
            instructions,
            function_count,
            import_count,
            export_count,
            total_gas_metric,
        })
    }

    /// Disassemble base64-encoded Soroban WASM bytecode.
    pub fn disassemble_base64(b64: &str) -> Result<DisassembledModule, XdrDecodeError> {
        let trimmed = b64.trim();
        let bytes = BASE64.decode(trimmed.as_bytes()).map_err(|e| XdrDecodeError::InvalidBase64 {
            offset: None,
            message: format!("base64 decode error: {e}"),
        })?;
        Self::disassemble_bytes(&bytes)
    }
}

fn estimate_op_gas(op_name: &str) -> u64 {
    match op_name {
        s if s.contains("Call") => 100,
        s if s.contains("Load") || s.contains("Store") => 30,
        s if s.contains("Div") || s.contains("Rem") => 20,
        s if s.contains("Mul") => 10,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::xdr::{ScVal, VecM, WriteXdr};

    #[test]
    fn decodes_return_value_and_v0_fees_from_xdr() {
        let meta = SorobanTransactionMeta {
            ext: SorobanTransactionMetaExt::V0,
            events: VecM::default(),
            return_value: ScVal::U32(42),
            diagnostic_events: VecM::default(),
        };
        let xdr = meta.to_xdr_base64(Limits::none()).unwrap();
        let decoded = XdrTransactionResultDecoder::decode_soroban_meta_xdr(&xdr).unwrap();
        assert_eq!(decoded.return_value, "42");
        assert_eq!(decoded.fees, ResourceFeeBreakdown::default());
    }

    #[test]
    fn rejects_invalid_base64_xdr_with_offset() {
        let error = XdrTransactionResultDecoder::decode_soroban_meta_xdr("not xdr").unwrap_err();
        assert!(matches!(error, XdrDecodeError::InvalidBase64 { .. }));
        let info = error.to_error_info();
        assert_eq!(info.kind, "base64");
        assert!(info.offset.is_some());
    }

    #[test]
    fn rejects_corrupt_payload_bytes_with_offset() {
        // Valid base64 encoding of random corrupt bytes (not valid SorobanTransactionMeta)
        let corrupt_base64 = BASE64.encode(&[0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE]);
        let error = XdrTransactionResultDecoder::decode_soroban_meta_xdr(&corrupt_base64).unwrap_err();
        assert!(matches!(error, XdrDecodeError::InvalidXdr { .. }));
        let info = error.to_error_info();
        assert_eq!(info.kind, "Soroban transaction metadata");
        assert!(info.offset.is_some());
    }

    #[test]
    fn rejects_corrupt_envelope_payload() {
        let corrupt_base64 = BASE64.encode(b"malformed_envelope_bytes");
        let error = XdrTransactionResultDecoder::decode_envelope(&corrupt_base64).unwrap_err();
        assert!(matches!(error, XdrDecodeError::InvalidXdr { .. }));
        let info = error.to_error_info();
        assert_eq!(info.kind, "transaction envelope");
        assert!(info.offset.is_some());
    }

    #[test]
    fn rejects_corrupt_result_meta_payload() {
        let corrupt_base64 = BASE64.encode(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let error = XdrTransactionResultDecoder::decode_result_meta(&corrupt_base64).unwrap_err();
        assert!(matches!(error, XdrDecodeError::InvalidXdr { .. }));
        let info = error.to_error_info();
        assert_eq!(info.kind, "transaction result metadata");
        assert!(info.offset.is_some());
    }

    #[test]
    fn handles_empty_or_whitespace_xdr_gracefully() {
        let error = XdrTransactionResultDecoder::decode_soroban_meta_xdr("   ").unwrap_err();
        assert!(matches!(error, XdrDecodeError::InvalidBase64 { .. }));
    }

    #[test]
    fn test_disassemble_wasm_bytes() {
        use wasm_encoder::{CodeSection, ExportKind, ExportSection, Function, FunctionSection, Module, TypeSection, ValType};
        let mut module = Module::new();
        let mut types = TypeSection::new();
        types.ty().function([], [ValType::I32]);
        module.section(&types);
        let mut functions = FunctionSection::new();
        functions.function(0);
        module.section(&functions);
        let mut exports = ExportSection::new();
        exports.export("add", ExportKind::Func, 0);
        module.section(&exports);
        let mut codes = CodeSection::new();
        let mut f = Function::new(vec![]);
        f.instruction(&wasm_encoder::Instruction::I32Const(42));
        f.instruction(&wasm_encoder::Instruction::End);
        codes.function(&f);
        module.section(&codes);
        let bytes = module.finish();

        let disasm = SorobanDisassembler::disassemble_bytes(&bytes).unwrap();
        assert!(disasm.wat.contains("(module"));
        assert_eq!(disasm.function_count, 1);
        assert_eq!(disasm.export_count, 1);
        assert!(!disasm.instructions.is_empty());
    }
}
