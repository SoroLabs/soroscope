use crate::simulation::SimulationError;
use serde::{Deserialize, Serialize};
use soroban_sdk::xdr::{
    FeeBumpTransactionInnerTx, HostFunction, Limits, OperationBody, ReadXdr, ScAddress,
    SorobanAuthorizationEntry, SorobanTransactionData, TransactionEnvelope, TransactionMeta,
    TransactionResult, WriteXdr,
};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct ResourceFeeBreakdown {
    pub total_fee_charged_stroops: i64,
    pub non_refundable_resource_fee_stroops: Option<i64>,
    pub refundable_resource_fee_stroops: Option<i64>,
    pub rent_fee_stroops: Option<i64>,
    pub inclusion_fee_stroops: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplaySource {
    Meta,
    Live,
}

#[derive(Debug, Clone)]
pub struct DecodedSorobanTransaction {
    pub host_function: HostFunction,
    pub auth_entries: Vec<SorobanAuthorizationEntry>,
    pub soroban_transaction_data: Option<SorobanTransactionData>,
    pub resource_fee_breakdown: ResourceFeeBreakdown,
    pub invocation: Option<DecodedInvocation>,
    pub original_meta_version: u32,
    pub skipped_operation_count: usize,
    pub replay_source: ReplaySource,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct DecodedInvocation {
    pub contract_id: String,
    pub function_name: String,
}

/// Decode the first Soroban host-function operation and its historical metadata.
pub fn decode_historical_transaction(
    envelope_xdr: &[u8],
    result_xdr: &[u8],
    result_meta_xdr: &[u8],
) -> Result<DecodedSorobanTransaction, SimulationError> {
    let envelope =
        TransactionEnvelope::from_xdr(envelope_xdr, Limits::none()).map_err(|error| {
            SimulationError::XdrError(format!("Invalid transaction envelope: {error}"))
        })?;
    let transaction = match envelope {
        TransactionEnvelope::TxV0(_) => return Err(SimulationError::NotSorobanTransaction),
        TransactionEnvelope::Tx(envelope) => envelope.tx,
        TransactionEnvelope::TxFeeBump(envelope) => match envelope.tx.inner_tx {
            FeeBumpTransactionInnerTx::Tx(inner) => inner.tx,
        },
    };

    let (operation_index, invoke) = transaction
        .operations
        .iter()
        .enumerate()
        .find_map(|(index, operation)| match &operation.body {
            OperationBody::InvokeHostFunction(invoke) => Some((index, invoke)),
            _ => None,
        })
        .ok_or(SimulationError::NotSorobanTransaction)?;
    let skipped_operation_count = transaction.operations.len() - operation_index - 1;

    let result = TransactionResult::from_xdr(result_xdr, Limits::none()).map_err(|error| {
        SimulationError::XdrError(format!("Invalid transaction result: {error}"))
    })?;
    let result_meta =
        TransactionMeta::from_xdr(result_meta_xdr, Limits::none()).map_err(|error| {
            SimulationError::XdrError(format!("Invalid transaction result meta: {error}"))
        })?;

    let (original_meta_version, soroban_meta) = match &result_meta {
        TransactionMeta::V0(_) => (0, None),
        TransactionMeta::V1(_) => (1, None),
        TransactionMeta::V2(_) => (2, None),
        TransactionMeta::V3(meta) => (3, meta.soroban_meta.as_ref()),
    };
    let (non_refundable, refundable, rent) = soroban_meta
        .and_then(|meta| match &meta.ext {
            soroban_sdk::xdr::SorobanTransactionMetaExt::V1(fees) => Some((
                fees.total_non_refundable_resource_fee_charged,
                fees.total_refundable_resource_fee_charged,
                fees.rent_fee_charged,
            )),
            soroban_sdk::xdr::SorobanTransactionMetaExt::V0 => None,
        })
        .map(|(non_refundable, refundable, rent)| {
            (Some(non_refundable), Some(refundable), Some(rent))
        })
        .unwrap_or((None, None, None));
    let inclusion_fee = non_refundable
        .zip(refundable)
        .map(|(non_refundable, refundable)| {
            result
                .fee_charged
                .saturating_sub(non_refundable.saturating_add(refundable))
        });

    let soroban_transaction_data = match &transaction.ext {
        soroban_sdk::xdr::TransactionExt::V0 => None,
        soroban_sdk::xdr::TransactionExt::V1(data) => Some(data.clone()),
    };
    let has_meta_footprint = soroban_transaction_data.as_ref().is_some_and(|data| {
        !data.resources.footprint.read_only.is_empty()
            || !data.resources.footprint.read_write.is_empty()
    });

    Ok(DecodedSorobanTransaction {
        host_function: invoke.host_function.clone(),
        auth_entries: invoke.auth.iter().cloned().collect(),
        soroban_transaction_data,
        resource_fee_breakdown: ResourceFeeBreakdown {
            total_fee_charged_stroops: result.fee_charged,
            non_refundable_resource_fee_stroops: non_refundable,
            refundable_resource_fee_stroops: refundable,
            rent_fee_stroops: rent,
            inclusion_fee_stroops: inclusion_fee,
        },
        invocation: decode_invocation(&invoke.host_function),
        original_meta_version,
        skipped_operation_count,
        replay_source: if has_meta_footprint {
            ReplaySource::Meta
        } else {
            ReplaySource::Live
        },
    })
}

fn decode_invocation(host_function: &HostFunction) -> Option<DecodedInvocation> {
    let HostFunction::InvokeContract(args) = host_function else {
        return None;
    };
    let ScAddress::Contract(hash) = &args.contract_address else {
        return None;
    };
    Some(DecodedInvocation {
        contract_id: stellar_strkey::Strkey::Contract(stellar_strkey::Contract(hash.0)).to_string(),
        function_name: args.function_name.to_string(),
    })
}

pub fn encode_xdr<T: WriteXdr>(value: &T) -> Result<Vec<u8>, SimulationError> {
    value
        .to_xdr(Limits::none())
        .map_err(|error| SimulationError::XdrError(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::xdr::{
        ExtensionPoint, Hash, InvokeContractArgs, InvokeHostFunctionOp, InvokeHostFunctionResult,
        LedgerEntryChange, LedgerEntryChanges, LedgerFootprint, LedgerKey, LedgerKeyContractCode,
        Memo, MuxedAccount, Operation, OperationBody, OperationMeta, OperationResult,
        OperationResultTr, Preconditions, ScVal, SequenceNumber, SorobanResources,
        SorobanTransactionData, SorobanTransactionMeta, SorobanTransactionMetaExt,
        SorobanTransactionMetaExtV1, Transaction, TransactionEnvelope, TransactionExt,
        TransactionMeta, TransactionMetaV3, TransactionResult, TransactionResultExt,
        TransactionResultResult, TransactionV1Envelope,
    };

    fn historical_fixture() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let host_function = HostFunction::InvokeContract(InvokeContractArgs {
            contract_address: ScAddress::Contract(Hash([0x42; 32])),
            function_name: "transfer".try_into().unwrap(),
            args: Vec::new().try_into().unwrap(),
        });
        let invoke_op = Operation {
            source_account: None,
            body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                host_function,
                auth: Vec::new().try_into().unwrap(),
            }),
        };
        let soroban_data = SorobanTransactionData {
            ext: ExtensionPoint::V0,
            resources: SorobanResources {
                footprint: LedgerFootprint {
                    read_only: vec![LedgerKey::ContractCode(LedgerKeyContractCode {
                        hash: Hash([0x11; 32]),
                    })]
                    .try_into()
                    .unwrap(),
                    read_write: Vec::new().try_into().unwrap(),
                },
                instructions: 1234,
                read_bytes: 4321,
                write_bytes: 111,
            },
            resource_fee: 140,
        };
        let transaction = Transaction {
            source_account: MuxedAccount::Ed25519(soroban_sdk::xdr::Uint256([0x22; 32])),
            fee: 200,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![invoke_op.clone(), invoke_op].try_into().unwrap(),
            ext: TransactionExt::V1(soroban_data),
        };
        let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: transaction,
            signatures: Vec::new().try_into().unwrap(),
        });

        let result = TransactionResult {
            fee_charged: 200,
            result: TransactionResultResult::TxSuccess(
                vec![
                    OperationResult::OpInner(OperationResultTr::InvokeHostFunction(
                        InvokeHostFunctionResult::Success(Hash([0x33; 32])),
                    )),
                    OperationResult::OpInner(OperationResultTr::InvokeHostFunction(
                        InvokeHostFunctionResult::Success(Hash([0x44; 32])),
                    )),
                ]
                .try_into()
                .unwrap(),
            ),
            ext: TransactionResultExt::V0,
        };

        let empty_changes: LedgerEntryChanges = Vec::<LedgerEntryChange>::new().try_into().unwrap();
        let meta = TransactionMeta::V3(TransactionMetaV3 {
            ext: ExtensionPoint::V0,
            tx_changes_before: empty_changes.clone(),
            operations: vec![
                OperationMeta {
                    changes: empty_changes.clone(),
                },
                OperationMeta {
                    changes: empty_changes.clone(),
                },
            ]
            .try_into()
            .unwrap(),
            tx_changes_after: empty_changes,
            soroban_meta: Some(SorobanTransactionMeta {
                ext: SorobanTransactionMetaExt::V1(SorobanTransactionMetaExtV1 {
                    ext: ExtensionPoint::V0,
                    total_non_refundable_resource_fee_charged: 100,
                    total_refundable_resource_fee_charged: 40,
                    rent_fee_charged: 10,
                }),
                events: Vec::new().try_into().unwrap(),
                return_value: ScVal::Void,
                diagnostic_events: Vec::new().try_into().unwrap(),
            }),
        });

        (
            encode_xdr(&envelope).unwrap(),
            encode_xdr(&result).unwrap(),
            encode_xdr(&meta).unwrap(),
        )
    }

    #[test]
    fn historical_fixture_decodes_invocation_fees_and_meta_footprint() {
        let (envelope, result, meta) = historical_fixture();
        let decoded = decode_historical_transaction(&envelope, &result, &meta).unwrap();

        let invocation = decoded.invocation.unwrap();
        assert_eq!(
            invocation.contract_id,
            stellar_strkey::Strkey::Contract(stellar_strkey::Contract([0x42; 32])).to_string()
        );
        assert_eq!(invocation.function_name, "transfer");
        assert_eq!(decoded.skipped_operation_count, 1);
        assert_eq!(decoded.original_meta_version, 3);
        assert_eq!(decoded.replay_source, ReplaySource::Meta);
        assert_eq!(
            decoded.resource_fee_breakdown.total_fee_charged_stroops,
            200
        );
        assert_eq!(
            decoded
                .resource_fee_breakdown
                .non_refundable_resource_fee_stroops,
            Some(100)
        );
        assert_eq!(
            decoded
                .resource_fee_breakdown
                .refundable_resource_fee_stroops,
            Some(40)
        );
        assert_eq!(decoded.resource_fee_breakdown.rent_fee_stroops, Some(10));
        assert_eq!(
            decoded.resource_fee_breakdown.inclusion_fee_stroops,
            Some(60)
        );
    }

    #[test]
    fn historical_classic_transaction_returns_typed_error() {
        let (envelope, _, _) = historical_fixture();
        let decoded = TransactionEnvelope::from_xdr(&envelope, Limits::none()).unwrap();
        let TransactionEnvelope::Tx(mut envelope) = decoded else {
            unreachable!()
        };
        envelope.tx.operations = Vec::new().try_into().unwrap();
        let classic_envelope = encode_xdr(&TransactionEnvelope::Tx(envelope)).unwrap();

        assert!(matches!(
            decode_historical_transaction(&classic_envelope, &[], &[]),
            Err(SimulationError::NotSorobanTransaction)
        ));
    }
}
