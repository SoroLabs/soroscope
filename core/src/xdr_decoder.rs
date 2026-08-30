//! Decodes Soroban transaction-result XDR into API-friendly diagnostics.

use serde::Serialize;
use soroban_sdk::xdr::{
    ContractEvent, ContractEventBody, HostFunction, Limits, Operation, OperationBody, ReadXdr,
    SorobanTransactionMeta, SorobanTransactionMetaExt, TransactionEnvelope, TransactionMeta,
    TransactionResultMeta,
};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DecodedTransactionResult {
    pub invocations: Vec<DecodedInvocation>,
    pub events: Vec<DecodedContractEvent>,
    pub diagnostics: Vec<DecodedDiagnosticEvent>,
    pub return_value: String,
    pub fees: ResourceFeeBreakdown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DecodedInvocation {
    /// Transaction source account in lossless XDR debug representation.
    pub invoker: String,
    pub contract: String,
    pub function: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DecodedContractEvent {
    pub contract_id: Option<String>,
    pub event_type: String,
    pub topics: Vec<String>,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DecodedDiagnosticEvent {
    pub in_successful_contract_call: bool,
    pub event: DecodedContractEvent,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct ResourceFeeBreakdown {
    pub non_refundable: i64,
    pub refundable: i64,
    pub rent: i64,
    pub total: i64,
}

#[derive(Debug, Error)]
pub enum XdrDecodeError {
    #[error("invalid {kind} XDR: {source}")]
    InvalidXdr {
        kind: &'static str,
        #[source]
        source: soroban_sdk::xdr::Error,
    },
    #[error("transaction metadata does not contain Soroban execution data")]
    MissingSorobanMetadata,
}

pub struct XdrTransactionResultDecoder;

impl XdrTransactionResultDecoder {
    /// Decodes the `resultMetaXdr` returned by Soroban RPC `getTransaction`.
    pub fn decode_result_meta(xdr: &str) -> Result<DecodedTransactionResult, XdrDecodeError> {
        let result = TransactionResultMeta::from_xdr_base64(xdr, Limits::none()).map_err(|source| {
            XdrDecodeError::InvalidXdr { kind: "transaction result metadata", source }
        })?;
        let meta = soroban_meta(&result.tx_apply_processing)
            .ok_or(XdrDecodeError::MissingSorobanMetadata)?;
        Ok(Self::decode_soroban_meta(meta))
    }

    /// Decodes standalone `SorobanTransactionMeta` XDR.
    pub fn decode_soroban_meta_xdr(xdr: &str) -> Result<DecodedTransactionResult, XdrDecodeError> {
        let meta = SorobanTransactionMeta::from_xdr_base64(xdr, Limits::none()).map_err(|source| {
            XdrDecodeError::InvalidXdr { kind: "Soroban transaction metadata", source }
        })?;
        Ok(Self::decode_soroban_meta(&meta))
    }

    /// Decodes host-function invocations from an RPC `envelopeXdr` value.
    pub fn decode_envelope(xdr: &str) -> Result<Vec<DecodedInvocation>, XdrDecodeError> {
        let envelope = TransactionEnvelope::from_xdr_base64(xdr, Limits::none()).map_err(|source| {
            XdrDecodeError::InvalidXdr { kind: "transaction envelope", source }
        })?;
        Ok(decode_envelope(&envelope))
    }

    /// Decodes a result and optionally enriches it with invocation details.
    pub fn decode(result_xdr: &str, envelope_xdr: Option<&str>) -> Result<DecodedTransactionResult, XdrDecodeError> {
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
                total: fees.total_non_refundable_resource_fee_charged + fees.total_refundable_resource_fee_charged,
            },
        };
        DecodedTransactionResult {
            invocations: Vec::new(),
            events: meta.events.iter().map(decode_event).collect(),
            diagnostics: meta.diagnostic_events.iter().map(|diagnostic| DecodedDiagnosticEvent {
                in_successful_contract_call: diagnostic.in_successful_contract_call,
                event: decode_event(&diagnostic.event),
            }).collect(),
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
        TransactionEnvelope::Tx(envelope) => decode_operations(&envelope.tx.source_account, &envelope.tx.operations),
        TransactionEnvelope::TxFeeBump(envelope) => match &envelope.tx.inner_tx {
            soroban_sdk::xdr::FeeBumpTransactionInnerTx::Tx(inner) => {
                decode_operations(&inner.tx.source_account, &inner.tx.operations)
            }
        },
        TransactionEnvelope::TxV0(_) => Vec::new(),
    }
}

fn decode_operations(source: &soroban_sdk::xdr::MuxedAccount, operations: &[Operation]) -> Vec<DecodedInvocation> {
    operations.iter().filter_map(|operation| match &operation.body {
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
    }).collect()
}

fn decode_event(event: &ContractEvent) -> DecodedContractEvent {
    let ContractEventBody::V0(body) = &event.body;
    DecodedContractEvent {
        contract_id: event.contract_id.as_ref().map(|id| hex::encode(id.0)),
        event_type: event.type_.to_string(),
        topics: body.topics.iter().map(format_sc_val).collect(),
        data: format_sc_val(&body.data),
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
        ScVal::Bytes(value) => format!("0x{}", hex::encode(value.as_ref())),
        _ => format!("{value:?}"),
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
    fn rejects_invalid_xdr() {
        let error = XdrTransactionResultDecoder::decode_soroban_meta_xdr("not xdr").unwrap_err();
        assert!(matches!(error, XdrDecodeError::InvalidXdr { .. }));
    }
}
