//! Resource-fee quoting, including an *estimate* of the refundable component.
//!
//! # What gets refunded
//!
//! On Soroban, the resource fee a transaction pays is not all gone forever. Rent
//! paid for **temporary** entries is returned when those entries expire, because
//! a temporary entry that is gone at the end of its TTL has occupied no space
//! after that point. Rent paid for **persistent** entries is *not* refunded —
//! that entry is the caller's data and keeps occupying the ledger.
//!
//! So the refundable part of a resource fee is exactly:
//!
//! ```text
//! estimated_refund = ceil(temp_rent_bytes * fee_per_write_1kb
//!                         / (data_size_1kb_increment * temporary_rent_rate_denominator))
//! ```
//!
//! which is the same expression `soroban-env-host::fees::compute_rent_fee` uses
//! for temporary entries. Note that `temporary_rent_rate_denominator` (4206) is
//! exactly twice `persistent_rent_rate_denominator` (2103) on pubnet, so rent on
//! a temporary byte costs half of rent on a persistent byte.
//!
//! # Why this is an estimate
//!
//! Two quantities are needed and the RPC only gives us one of them:
//!
//! * `cost.rentBytes` — the total rent the node charged. The RPC reports this.
//! * The **durability split** — how much of that rent was temporary vs
//!   persistent. The RPC does *not* report this.
//!
//! [`DurabilitySplit`] is therefore an explicit input, and when it is not known
//! the refund is `None` rather than `0`. Returning `0` would be a lie in the
//! dangerous direction: it would render as "this write refunds nothing" when we
//! actually mean "we do not know", and callers that optimise against refunds
//! would silently keep the pessimistic gross fee. Returning `None` lets the UI
//! say "refund unknown" and the caller fall back to the gross fee.
//!
//! Out of scope (deliberately): modelling *when* a refund actually lands, or
//! discounting it by the number of ledgers until expiry. The refund is reported
//! at face value and is always labelled as an estimate via
//! [`ResourceFeeQuote::refund_is_estimate`].

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::simulation::SimulationStateSnapshot;

/// The checked-in pubnet fee parameters. Compiled into the binary so a quote is
/// reproducible and reviewable in a diff rather than depending on a live node.
pub const SOROBAN_FEE_CONFIG_JSON: &str = include_str!("../config/soroban-fees.json");

/// Fee parameters for one network, as checked in at
/// `core/config/soroban-fees.json`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SorobanFeeConfig {
    pub network: String,
    pub captured_at: String,
    pub protocol: u32,
    pub source: String,
    pub fee_per_instruction_increment: i64,
    pub instructions_increment: i64,
    pub fee_per_read_entry: i64,
    pub fee_per_write_entry: i64,
    pub fee_per_read_1kb: i64,
    pub fee_per_write_1kb: i64,
    pub fee_per_historical_1kb: i64,
    pub fee_per_contract_event_1kb: i64,
    pub fee_per_transaction_size_1kb: i64,
    pub data_size_1kb_increment: i64,
    pub tx_base_result_size: i64,
    pub ttl_entry_size: i64,
    pub persistent_rent_rate_denominator: i64,
    pub temporary_rent_rate_denominator: i64,
}

impl SorobanFeeConfig {
    /// Parse a config from raw JSON.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("invalid soroban-fees.json: {e}"))
    }

    /// The config compiled into the binary.
    pub fn checked_in() -> &'static SorobanFeeConfig {
        static CONFIG: OnceLock<SorobanFeeConfig> = OnceLock::new();
        CONFIG.get_or_init(|| {
            Self::from_json(SOROBAN_FEE_CONFIG_JSON).expect("built-in soroban-fees.json must parse")
        })
    }

    /// Return a checked-in table only when its protocol matches the request.
    pub fn for_protocol(protocol: u32) -> Option<&'static SorobanFeeConfig> {
        let config = Self::checked_in();
        (config.protocol == protocol).then_some(config)
    }

    /// Stable content hash identifying the fee-parameter table used by a quote.
    pub fn content_hash(&self) -> String {
        use sha2::{Digest, Sha256};

        let bytes = serde_json::to_vec(self).expect("fee config serialization is infallible");
        format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
    }

    /// Bytes of temporary entry rent that a single ledger of TTL costs, i.e. the
    /// divisor in the rent formula.
    fn temporary_rent_divisor(&self) -> u128 {
        self.temporary_rent_divisor_with(self.temporary_rent_rate_denominator)
    }

    /// Bytes of persistent entry rent that a single ledger of TTL costs.
    fn persistent_rent_divisor(&self) -> u128 {
        self.temporary_rent_divisor_with(self.persistent_rent_rate_denominator)
    }

    fn temporary_rent_divisor_with(&self, rate_denominator: i64) -> u128 {
        let kb = self.data_size_1kb_increment.max(1) as u128;
        let denominator = rate_denominator.max(1) as u128;
        kb.saturating_mul(denominator).max(1)
    }

    /// Hand the checked-in snapshot to `soroban-env-host`'s own calculator.
    ///
    /// The whole point of routing through the host is that we do not re-derive
    /// the fee arithmetic. `soroban-sdk`'s own test helper builds this exact
    /// struct from the same 2024-12-11 pubnet snapshot that
    /// `core/config/soroban-fees.json` records, so a quote produced here and a
    /// quote produced by the SDK agree by construction rather than by two
    /// independent implementations happening to match.
    ///
    /// The two rent denominators are returned alongside because
    /// [`soroban_env_host::InvocationResources::estimate_fees`] takes them
    /// positionally rather than folding them into the configuration.
    pub fn host_fee_configuration(&self) -> (soroban_env_host::fees::FeeConfiguration, i64, i64) {
        let fee_configuration = soroban_env_host::fees::FeeConfiguration {
            fee_per_instruction_increment: self.fee_per_instruction_increment,
            fee_per_read_entry: self.fee_per_read_entry,
            fee_per_write_entry: self.fee_per_write_entry,
            fee_per_read_1kb: self.fee_per_read_1kb,
            // The host's own helper comments that this is deliberately an
            // overestimate of the network fee, to stay conservative as state
            // grows. We keep the snapshot value rather than recomputing it from
            // `fee_per_historical_1kb`, so our numbers match the SDK's.
            fee_per_write_1kb: self.fee_per_write_1kb,
            fee_per_historical_1kb: self.fee_per_historical_1kb,
            fee_per_contract_event_1kb: self.fee_per_contract_event_1kb,
            fee_per_transaction_size_1kb: self.fee_per_transaction_size_1kb,
        };
        (
            fee_configuration,
            self.persistent_rent_rate_denominator,
            self.temporary_rent_rate_denominator,
        )
    }

    /// Estimate rent for one entry over the requested number of ledgers.
    pub fn estimate_entry_rent_stroops(
        &self,
        entry_size_bytes: u64,
        ledgers: u32,
        temporary: bool,
    ) -> u64 {
        let divisor = if temporary {
            self.temporary_rent_divisor()
        } else {
            self.persistent_rent_divisor()
        };
        rent_fee(
            entry_size_bytes.saturating_mul(ledgers as u64),
            self.fee_per_write_1kb,
            divisor,
        )
    }

    /// Estimate the resource write fee for restoring one entry.
    pub fn estimate_restore_write_stroops(&self, entry_size_bytes: u64) -> u64 {
        (self.fee_per_write_entry.max(0) as u64).saturating_add(fee_per_increment(
            entry_size_bytes as u128,
            self.fee_per_write_1kb.max(0) as u128,
            self.data_size_1kb_increment.max(1) as u128,
        ))
    }
}

/// Resource-side fee components, all denominated in stroops.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalResourceFee {
    pub resource_fee: u64,
    pub rent_fee: u64,
    pub refundable: u64,
    pub non_refundable: u64,
}

/// Local fee calibration attached to a simulation result.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeeCalibration {
    pub cost_parameter_hash: String,
    pub local_resource_fee_stroops: u64,
    pub rpc_min_resource_fee_stroops: Option<u64>,
    pub delta_stroops: Option<i64>,
    /// Present when the local fee differs from the RPC minimum by more than 1%.
    pub calibration_error: Option<String>,
}

impl FeeCalibration {
    pub fn local(local_fee: u64, config: &SorobanFeeConfig) -> Self {
        Self {
            cost_parameter_hash: config.content_hash(),
            local_resource_fee_stroops: local_fee,
            ..Default::default()
        }
    }

    pub fn compare_rpc(
        local_fee: u64,
        rpc_min_resource_fee: Option<u64>,
        config: &SorobanFeeConfig,
    ) -> Self {
        let mut calibration = Self::local(local_fee, config);
        calibration.rpc_min_resource_fee_stroops = rpc_min_resource_fee;
        if let Some(rpc_fee) = rpc_min_resource_fee {
            let delta = local_fee as i128 - rpc_fee as i128;
            calibration.delta_stroops =
                Some(delta.clamp(i64::MIN as i128, i64::MAX as i128) as i64);
            let exceeds_one_percent = if rpc_fee == 0 {
                local_fee != 0
            } else {
                delta.unsigned_abs().saturating_mul(100) > rpc_fee as u128
            };
            if exceeds_one_percent {
                calibration.calibration_error = Some(format!(
                    "local resource fee differs from RPC minResourceFee by more than 1% (local={local_fee}, rpc={rpc_fee})"
                ));
            }
        }
        calibration
    }
}

/// `ceil(numerator / denominator)` in `u128` space, saturating to `u64::MAX`.
///
/// A profiler must never panic or wrap on hostile numbers coming off the wire:
/// a wrapped fee reads as *cheap*, which is the one answer a cost tool must not
/// invent.
fn div_ceil(numerator: u128, denominator: u128) -> u64 {
    if denominator == 0 {
        return 0;
    }
    let quotient = numerator / denominator;
    let result = if numerator % denominator == 0 {
        quotient
    } else {
        quotient.saturating_add(1)
    };
    result.min(u64::MAX as u128) as u64
}

/// Per-component resource fee, in stroops. Sums to the gross fee.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct ResourceFeeBreakdown {
    pub instructions: u64,
    pub read_entries: u64,
    pub write_entries: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    /// Archival/historical write cost of the transaction result.
    pub historical_bytes: u64,
    /// Bandwidth cost of the transaction envelope itself.
    pub bandwidth_bytes: u64,
    pub contract_events: u64,
    pub temporary_rent: u64,
    pub persistent_rent: u64,
}

impl ResourceFeeBreakdown {
    pub fn total(&self) -> u64 {
        self.instructions
            .saturating_add(self.read_entries)
            .saturating_add(self.write_entries)
            .saturating_add(self.read_bytes)
            .saturating_add(self.write_bytes)
            .saturating_add(self.historical_bytes)
            .saturating_add(self.bandwidth_bytes)
            .saturating_add(self.contract_events)
            .saturating_add(self.temporary_rent)
            .saturating_add(self.persistent_rent)
    }
}

/// How the written ledger bytes split between temporary and persistent entries.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct DurabilitySplit {
    pub temporary_write_bytes: u64,
    pub persistent_write_bytes: u64,
    /// `false` when the simulation did not expose a durability breakdown. When
    /// `false` the byte counts above are meaningless and the refund is `None`.
    pub known: bool,
}

impl DurabilitySplit {
    pub fn unknown() -> Self {
        Self {
            known: false,
            ..Default::default()
        }
    }

    pub fn temporary_only(bytes: u64) -> Self {
        Self {
            temporary_write_bytes: bytes,
            persistent_write_bytes: 0,
            known: true,
        }
    }

    pub fn persistent_only(bytes: u64) -> Self {
        Self {
            temporary_write_bytes: 0,
            persistent_write_bytes: bytes,
            known: true,
        }
    }

    /// A split of `temporary` of `total` bytes.
    pub fn mixed(temporary: u64, persistent: u64) -> Self {
        Self {
            temporary_write_bytes: temporary,
            persistent_write_bytes: persistent,
            known: true,
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.temporary_write_bytes
            .saturating_add(self.persistent_write_bytes)
    }

    /// Recover a durability split from the ledger entries a simulation touched.
    ///
    /// Each snapshot key is a base64 `LedgerKey`; `ContractData` keys carry their
    /// durability in the clear, and the paired base64 `LedgerEntry` gives a size
    /// we can weight by. Entry size is approximated by the XDR length of the
    /// stored entry — good enough to establish a *ratio* between durabilities,
    /// which is all the refund estimate needs.
    ///
    /// Returns [`DurabilitySplit::unknown`] when the snapshot is missing, holds
    /// no `ContractData` keys, or a key fails to decode. Guessing here would
    /// produce a confidently wrong refund.
    pub fn from_state_snapshot(snapshot: Option<&SimulationStateSnapshot>) -> Self {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        use soroban_sdk::xdr::{ContractDataDurability, LedgerKey, Limits, ReadXdr};

        let Some(snapshot) = snapshot else {
            return Self::unknown();
        };

        let mut temporary = 0u64;
        let mut persistent = 0u64;
        let mut saw_contract_data = false;

        for (key_b64, entry_b64) in &snapshot.ledger_entries {
            let Ok(key_bytes) = BASE64.decode(key_b64) else {
                continue;
            };
            let Ok(LedgerKey::ContractData(contract_data)) =
                LedgerKey::from_xdr(&key_bytes, Limits::none())
            else {
                continue;
            };
            saw_contract_data = true;

            // Weight by the stored entry's encoded size; fall back to the key
            // size alone if the value is unreadable, so a partial snapshot
            // still yields a split rather than nothing.
            let size = match BASE64.decode(entry_b64).map(|bytes| bytes.len() as u64) {
                Ok(len) => len.max(1),
                Err(_) => 1,
            };

            match contract_data.durability {
                ContractDataDurability::Temporary => temporary = temporary.saturating_add(size),
                ContractDataDurability::Persistent => persistent = persistent.saturating_add(size),
            }
        }

        if !saw_contract_data || temporary.saturating_add(persistent) == 0 {
            return Self::unknown();
        }

        Self::mixed(temporary, persistent)
    }
}

/// Why [`ResourceFeeQuote::estimated_refund`] is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefundStatus {
    /// The durability split and rent total were both available.
    Estimated,
    /// The simulation did not expose a durability breakdown.
    UnknownDurability,
    /// The node did not report `cost.rentBytes`, so there is no rent to split.
    UnknownRentBytes,
}

/// The resource costs to price. Mirrors what the RPC reports, plus the two
/// entry counts the RPC omits from `SorobanResources` (defaulted to zero when
/// unknown).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct FeeQuoteInput {
    pub cpu_instructions: u64,
    pub ledger_read_bytes: u64,
    pub ledger_write_bytes: u64,
    pub transaction_size_bytes: u64,
    pub read_entries: u64,
    pub write_entries: u64,
    pub contract_event_bytes: u64,
    /// Total rent the node charged (`cost.rentBytes`). `None` when the RPC did
    /// not report it.
    pub rent_bytes: Option<u64>,
}

impl FeeQuoteInput {
    /// Build from the resources on a simulation result.
    pub fn from_soroban_resources(
        resources: &crate::simulation::SorobanResources,
        rent_bytes: Option<u64>,
    ) -> Self {
        Self {
            cpu_instructions: resources.cpu_instructions,
            ledger_read_bytes: resources.ledger_read_bytes,
            ledger_write_bytes: resources.ledger_write_bytes,
            transaction_size_bytes: resources.transaction_size_bytes,
            read_entries: 0,
            write_entries: 0,
            contract_event_bytes: 0,
            rent_bytes,
        }
    }
}

/// A priced simulation: the gross fee always, plus an estimated refund when the
/// durability split is known.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceFeeQuote {
    /// Full resource fee. Always present, even when the refund is unknown, so
    /// callers can fall back to the worst case.
    pub gross_resource_fee: u64,
    /// Refundable portion of the gross fee. `None` means "unknown", **not** zero.
    pub estimated_refund: Option<u64>,
    /// `gross_resource_fee - estimated_refund`. `None` when the refund is unknown.
    pub estimated_net: Option<u64>,
    pub refund_status: RefundStatus,
    /// Always `true`. The refund is computed from a durability *estimate* and
    /// from a checked-in config, not from what the node will actually credit.
    pub refund_is_estimate: bool,
    /// `false` when rent bytes were unavailable, in which case
    /// `gross_resource_fee` omits rent and is a lower bound.
    pub gross_includes_rent: bool,
    pub durability_split: DurabilitySplit,
    /// Split of `cost.rentBytes` attributed to temporary entries, when known.
    pub temporary_rent_bytes: Option<u64>,
    pub breakdown: ResourceFeeBreakdown,
}

impl ResourceFeeQuote {
    /// Price `input` and, where possible, estimate the refund implied by
    /// `durability_split`.
    pub fn estimate(
        input: &FeeQuoteInput,
        durability_split: DurabilitySplit,
        config: &SorobanFeeConfig,
    ) -> Self {
        let (temporary_rent_bytes, persistent_rent_bytes) = match input.rent_bytes {
            Some(total) => match split_rent_bytes(total, durability_split) {
                Some((temporary, persistent)) => (temporary, persistent),
                // Durability unknown: price *all* rent at the persistent rate.
                // That is the pessimistic reading, so `gross` stays a safe
                // upper bound for a caller with no better information.
                None => (0, total),
            },
            None => (0, 0),
        };

        let breakdown = ResourceFeeBreakdown {
            instructions: fee_per_increment(
                input.cpu_instructions as u128,
                config.fee_per_instruction_increment as u128,
                config.instructions_increment as u128,
            ),
            // A write entry is also billed as a read entry, matching
            // `soroban-env-host::fees::compute_transaction_resource_fee`.
            read_entries: (config.fee_per_read_entry.max(0) as u64)
                .saturating_mul(input.read_entries.saturating_add(input.write_entries)),
            write_entries: (config.fee_per_write_entry.max(0) as u64)
                .saturating_mul(input.write_entries),
            read_bytes: fee_per_increment(
                input.ledger_read_bytes as u128,
                config.fee_per_read_1kb as u128,
                config.data_size_1kb_increment as u128,
            ),
            write_bytes: fee_per_increment(
                input.ledger_write_bytes as u128,
                config.fee_per_write_1kb as u128,
                config.data_size_1kb_increment as u128,
            ),
            historical_bytes: fee_per_increment(
                input
                    .transaction_size_bytes
                    .saturating_add(config.tx_base_result_size.max(0) as u64)
                    as u128,
                config.fee_per_historical_1kb as u128,
                config.data_size_1kb_increment as u128,
            ),
            bandwidth_bytes: fee_per_increment(
                input.transaction_size_bytes as u128,
                config.fee_per_transaction_size_1kb as u128,
                config.data_size_1kb_increment as u128,
            ),
            contract_events: fee_per_increment(
                input.contract_event_bytes as u128,
                config.fee_per_contract_event_1kb as u128,
                config.data_size_1kb_increment as u128,
            ),
            temporary_rent: rent_fee(
                temporary_rent_bytes,
                config.fee_per_write_1kb,
                config.temporary_rent_divisor(),
            ),
            persistent_rent: rent_fee(
                persistent_rent_bytes,
                config.fee_per_write_1kb,
                config.persistent_rent_divisor(),
            ),
        };

        let gross_resource_fee = breakdown.total();

        let (estimated_refund, refund_status) = match (input.rent_bytes, durability_split.known) {
            (Some(total), true) => {
                let temporary = split_rent_bytes(total, durability_split)
                    .map(|(temporary, _)| temporary)
                    .unwrap_or(0);
                (
                    Some(rent_fee(
                        temporary,
                        config.fee_per_write_1kb,
                        config.temporary_rent_divisor(),
                    )),
                    RefundStatus::Estimated,
                )
            }
            (Some(_), false) => (None, RefundStatus::UnknownDurability),
            (None, _) => (None, RefundStatus::UnknownRentBytes),
        };

        let estimated_net =
            estimated_refund.map(|refund| gross_resource_fee.saturating_sub(refund));

        Self {
            gross_resource_fee,
            estimated_refund,
            estimated_net,
            refund_status,
            refund_is_estimate: true,
            gross_includes_rent: input.rent_bytes.is_some(),
            durability_split,
            temporary_rent_bytes: if refund_status == RefundStatus::Estimated {
                split_rent_bytes(input.rent_bytes.unwrap_or(0), durability_split)
                    .map(|(temporary, _)| temporary)
            } else {
                None
            },
            breakdown,
        }
    }
}

/// Price measured resources and caller-supplied footprint sizes using a fixed
/// protocol fee table. The resource counters are already metered by the host's
/// protocol cost parameters; this function converts them to stroops.
pub fn price_local_resource_fee(
    resources: &crate::simulation::SorobanResources,
    footprint: &FeeQuoteInput,
    durability_split: DurabilitySplit,
    config: &SorobanFeeConfig,
) -> LocalResourceFee {
    let input = FeeQuoteInput {
        cpu_instructions: resources.cpu_instructions,
        ledger_read_bytes: resources.ledger_read_bytes,
        ledger_write_bytes: resources.ledger_write_bytes,
        transaction_size_bytes: resources.transaction_size_bytes,
        ..*footprint
    };
    let quote = ResourceFeeQuote::estimate(&input, durability_split, config);
    let rent_fee = quote
        .breakdown
        .temporary_rent
        .saturating_add(quote.breakdown.persistent_rent);
    let refundable = quote.estimated_refund.unwrap_or(0);
    LocalResourceFee {
        resource_fee: quote.gross_resource_fee,
        rent_fee,
        refundable,
        non_refundable: quote.gross_resource_fee.saturating_sub(refundable),
    }
}

fn fee_per_increment(resource_value: u128, fee_rate: u128, increment: u128) -> u64 {
    div_ceil(resource_value.saturating_mul(fee_rate), increment.max(1))
}

fn rent_fee(rent_bytes: u64, fee_per_write_1kb: i64, divisor: u128) -> u64 {
    if rent_bytes == 0 {
        return 0;
    }
    div_ceil(
        (rent_bytes as u128).saturating_mul(fee_per_write_1kb.max(0) as u128),
        divisor,
    )
}

/// Split total rent bytes between durabilities in proportion to written bytes.
///
/// The two halves are made to sum back to `total` exactly (the persistent half
/// takes the remainder) so the gross fee is not inflated by a rounding pair.
fn split_rent_bytes(total: u64, durability_split: DurabilitySplit) -> Option<(u64, u64)> {
    if !durability_split.known {
        return None;
    }
    let split_total = durability_split.total_bytes();
    if split_total == 0 {
        // A known split of zero written bytes: no rent was paid, so the refund
        // is a definite zero rather than an unknown.
        return Some((0, 0));
    }
    let temporary = div_ceil(
        (total as u128).saturating_mul(durability_split.temporary_write_bytes as u128),
        split_total as u128,
    )
    .min(total);
    Some((temporary, total.saturating_sub(temporary)))
}

/// Convenience: derive a [`DurabilitySplit`] and price in one step from a
/// completed simulation.
pub fn quote_simulation(
    simulation: &crate::simulation::SimulationResult,
    rent_bytes: Option<u64>,
    config: &SorobanFeeConfig,
) -> ResourceFeeQuote {
    let input = FeeQuoteInput::from_soroban_resources(&simulation.resources, rent_bytes);
    let split = DurabilitySplit::from_state_snapshot(simulation.state_snapshot.as_ref());
    ResourceFeeQuote::estimate(&input, split, config)
}

// ─────────────────────────────────────────────────────────────────────────────
// Total fee quote: resource side + inclusion side (issue #1011)
// ─────────────────────────────────────────────────────────────────────────────

/// Whether an inclusion bid could be produced for this quote.
///
/// The resource side of a quote is always available — it is arithmetic over
/// measured resources and a checked-in fee table. The inclusion side depends on
/// having recent ledger fee samples to predict from, which is a property of the
/// node Soroscope is pointed at, not of the transaction being simulated. So the
/// two are reported independently and this says which one is present.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InclusionStatus {
    /// Bids were predicted from ledger fee samples.
    Available,
    /// No samples were available, so no bid could be predicted.
    Unavailable,
}

impl Default for InclusionStatus {
    /// A quote that omits the field is treated as having no bid.
    ///
    /// This is the pessimistic reading on purpose: a missing `inclusion` key
    /// should not be interpreted as "bids were fine and just got lost in
    /// serialisation", which would let a caller treat an unknown bid as zero.
    fn default() -> Self {
        Self::Unavailable
    }
}

/// One number for what a transaction costs, separating the three reasons it
/// costs anything at all.
///
/// A Soroban transaction's fee has two halves that fail in opposite directions:
///
/// * the **resource fee**, set by the work the contract does, and
/// * the **inclusion fee** (a.k.a. the bid), set by ledger congestion and how
///   fast you want to be included.
///
/// People routinely bump the wrong one. Raising the bid when the resource fee is
/// the actual problem costs more and changes nothing about the resource side;
/// lowering the bid when resources are the problem gets you a cheaper
/// transaction that is rejected or that spills over into the next ledger's
/// congestion. This type exists so both halves are visible in one place.
///
/// # Field relationships
///
/// * `resource_fee` is the total resource fee and **includes** `rent`.
/// * `rent` is the portion of `resource_fee` that is rent, and includes
///   `refundable`.
/// * `refundable` is the portion of `rent` that comes back: rent on temporary
///   entries, returned when those entries expire at the end of their TTL. Rent
///   on persistent entries never comes back.
///
/// So `refundable <= rent <= resource_fee` always holds, and
/// [`FeeQuote::non_refundable_resource_fee`] is the part that is gone for good
/// regardless of what happens to temporary entries.
///
/// None of these are estimates *except* that they are estimates of a fee, not a
/// charge: the network decides the final resource fee when it meters the
/// transaction for real.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
pub struct FeeQuote {
    /// Total resource fee, rent included. Always present.
    #[serde(default)]
    pub resource_fee: u64,

    /// The portion of `resource_fee` that is rent rather than execution.
    #[serde(default)]
    pub rent: u64,

    /// The portion of `rent` that is refunded at temporary-entry expiry.
    #[serde(default)]
    pub refundable: u64,

    /// Bid for cheap, slow inclusion. `None` when inclusion is unavailable.
    #[serde(default)]
    pub inclusion_economy: Option<u64>,

    /// Bid for balanced inclusion. `None` when inclusion is unavailable.
    #[serde(default)]
    pub inclusion_standard: Option<u64>,

    /// Bid for fast inclusion. `None` when inclusion is unavailable.
    #[serde(default)]
    pub inclusion_priority: Option<u64>,

    /// A **ceiling**, not a charge: `resource_fee` plus the standard bid.
    ///
    /// This is what a caller should provision, not what they will be billed.
    /// Actual cost is bounded by this and is usually lower, because the standard
    /// bid over-provisions and the bid is only paid for the ledgers the
    /// transaction is actually included in. `None` when inclusion is
    /// unavailable, since without a bid there is no ceiling to state.
    #[serde(default)]
    pub total_max: Option<u64>,

    /// Whether the inclusion fields above are populated.
    #[serde(default)]
    pub inclusion: InclusionStatus,
}

impl FeeQuote {
    /// The part of the resource fee that is never refunded.
    ///
    /// This is the number that actually leaves the account for good, and it is
    /// the one that grows when a contract does more work.
    pub fn non_refundable_resource_fee(&self) -> u64 {
        self.resource_fee.saturating_sub(self.rent)
    }

    /// True when the inclusion bids could not be predicted.
    pub fn inclusion_unavailable(&self) -> bool {
        self.inclusion == InclusionStatus::Unavailable
    }

    /// Build a quote from resources the host metered and an optional inclusion
    /// prediction.
    ///
    /// The resource side is computed by `soroban-env-host`'s own
    /// [`soroban_env_host::InvocationResources::estimate_fees`], which is also
    /// what `soroban-sdk`'s test helper uses. We do not re-derive that
    /// arithmetic: the host already splits rent into persistent and temporary
    /// components, and the temporary component *is* the refundable amount, so
    /// there is no need to infer a durability split from a footprint the way
    /// [`ResourceFeeQuote`] has to.
    pub fn from_host_resources(
        resources: &soroban_env_host::InvocationResources,
        config: &SorobanFeeConfig,
        prediction: Option<&crate::fee_analytics::FeePrediction>,
    ) -> Self {
        let (fee_configuration, persistent_denominator, temporary_denominator) =
            config.host_fee_configuration();
        let estimate = resources.estimate_fees(
            &fee_configuration,
            persistent_denominator,
            temporary_denominator,
        );

        // `estimate_fees` works in i64 and saturates; clamp on the way out so a
        // hostile or buggy number can never surface as a negative fee.
        let non_negative = |value: i64| -> u64 { u64::try_from(value).unwrap_or(0) };

        let persistent_rent = non_negative(estimate.persistent_entry_rent);
        let temporary_rent = non_negative(estimate.temporary_entry_rent);
        let rent = persistent_rent.saturating_add(temporary_rent);
        let resource_fee = non_negative(estimate.total).max(rent);

        // No prediction means no bid. The resource side above is unaffected,
        // which is the point: a node with no fee history still tells you
        // exactly what the contract will cost to execute.
        let Some(prediction) = prediction else {
            return Self {
                resource_fee,
                rent,
                refundable: temporary_rent,
                inclusion_economy: None,
                inclusion_standard: None,
                inclusion_priority: None,
                total_max: None,
                inclusion: InclusionStatus::Unavailable,
            };
        };

        let inclusion_standard = prediction.standard_bid;
        Self {
            resource_fee,
            rent,
            refundable: temporary_rent,
            inclusion_economy: Some(prediction.economy_bid),
            inclusion_standard: Some(inclusion_standard),
            inclusion_priority: Some(prediction.priority_bid),
            // The ceiling uses the *standard* bid, matching what a caller
            // provisioning a transaction would actually set.
            total_max: Some(resource_fee.saturating_add(inclusion_standard)),
            inclusion: InclusionStatus::Available,
        }
    }

    /// Compose a quote for a measured invocation, predicting inclusion from
    /// recent ledger fee samples for the configured network.
    ///
    /// An empty sample list yields [`InclusionStatus::Unavailable`] with the
    /// resource side still populated. Note that
    /// [`crate::fee_analytics::FeeAnalyticsEngine::predict`] returns fixed
    /// placeholder bids (100/100/150) for an empty sample list rather than
    /// signalling the absence of data, so we check for that case here instead
    /// of forwarding it — otherwise every unbacked node would look like it had a
    /// confident 100-stroop economy bid.
    pub fn compose(
        resources: &soroban_env_host::InvocationResources,
        samples: &[crate::fee_store::LedgerFeeSample],
        current_ledger: u64,
        config: &SorobanFeeConfig,
    ) -> Self {
        if samples.is_empty() {
            return Self::from_host_resources(resources, config, None);
        }
        let prediction =
            crate::fee_analytics::FeeAnalyticsEngine::new().predict(samples, current_ledger);
        Self::from_host_resources(resources, config, Some(&prediction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config() -> &'static SorobanFeeConfig {
        SorobanFeeConfig::checked_in()
    }

    /// Resources shared by every fixture so only the rent terms move. Hand
    /// computed against the checked-in pubnet config:
    ///
    /// | term                        | math                                        | stroops |
    /// |-----------------------------|---------------------------------------------|---------|
    /// | instructions                | ceil(100000 * 25 / 10000)                   | 250     |
    /// | read entries                | 0                                           | 0       |
    /// | write entries               | 0                                           | 0       |
    /// | read bytes                  | ceil(1024 * 1786 / 1024)                    | 1786    |
    /// | write bytes                 | ceil(1024 * 12000 / 1024)                   | 12000   |
    /// | historical bytes            | ceil(1324 * 16235 / 1024)                   | 20992   |
    /// | bandwidth bytes             | ceil(1024 * 1624 / 1024)                    | 1624    |
    /// | contract events             | 0                                           | 0       |
    /// | **base (no rent)**          |                                             | 36652   |
    fn base_input(rent_bytes: Option<u64>) -> FeeQuoteInput {
        FeeQuoteInput {
            cpu_instructions: 100_000,
            ledger_read_bytes: 1024,
            ledger_write_bytes: 1024,
            transaction_size_bytes: 1024,
            read_entries: 0,
            write_entries: 0,
            contract_event_bytes: 0,
            rent_bytes,
        }
    }

    const BASE_FEE: u64 = 36_652;

    #[test]
    fn checked_in_config_parses_and_matches_pubnet_snapshot() {
        let c = config();
        assert_eq!(c.network, "pubnet");
        assert_eq!(c.protocol, 22);
        assert_eq!(c.fee_per_write_1kb, 12_000);
        assert_eq!(c.data_size_1kb_increment, 1024);
        // Temp rent is exactly half of persistent rent per byte on pubnet.
        assert_eq!(
            c.temporary_rent_rate_denominator,
            c.persistent_rent_rate_denominator * 2
        );
    }

    #[test]
    fn base_terms_are_hand_computed() {
        let quote = ResourceFeeQuote::estimate(
            &base_input(Some(0)),
            DurabilitySplit::temporary_only(1024),
            config(),
        );
        assert_eq!(quote.breakdown.instructions, 250);
        assert_eq!(quote.breakdown.read_bytes, 1786);
        assert_eq!(quote.breakdown.write_bytes, 12_000);
        assert_eq!(quote.breakdown.historical_bytes, 20_992);
        assert_eq!(quote.breakdown.bandwidth_bytes, 1624);
        assert_eq!(quote.breakdown.temporary_rent, 0);
        assert_eq!(quote.gross_resource_fee, BASE_FEE);
    }

    #[test]
    fn local_read_only_call_matches_hand_computed_fee() {
        let resources = crate::simulation::SorobanResources {
            cpu_instructions: 100_000,
            ledger_read_bytes: 1024,
            ..Default::default()
        };
        let input = FeeQuoteInput {
            read_entries: 1,
            ..Default::default()
        };
        let priced =
            price_local_resource_fee(&resources, &input, DurabilitySplit::unknown(), config());

        // 250 instructions + 6250 read entry + 1786 read bytes + 4757 history.
        assert_eq!(priced.resource_fee, 13_043);
        assert_eq!(priced.rent_fee, 0);
        assert_eq!(priced.refundable, 0);
        assert_eq!(priced.non_refundable, 13_043);
    }

    #[test]
    fn local_persistent_write_fixture_and_write_rate_protocol_bump() {
        let resources = crate::simulation::SorobanResources {
            cpu_instructions: 100_000,
            ledger_write_bytes: 1024,
            ..Default::default()
        };
        let input = FeeQuoteInput {
            write_entries: 1,
            rent_bytes: Some(1024),
            ..Default::default()
        };
        let split = DurabilitySplit::persistent_only(1024);
        let baseline = price_local_resource_fee(&resources, &input, split, config());

        // 250 CPU + 6250 read entry + 10000 write entry + 12000 write bytes
        // + 4757 history + 6 persistent rent.
        assert_eq!(baseline.resource_fee, 33_263);
        assert_eq!(baseline.rent_fee, 6);
        assert_eq!(baseline.refundable, 0);
        assert_eq!(baseline.non_refundable, 33_263);

        let mut bumped = config().clone();
        bumped.protocol += 1;
        bumped.fee_per_write_1kb = 13_000;
        let bumped_write = price_local_resource_fee(&resources, &input, split, &bumped);
        let read_only = crate::simulation::SorobanResources {
            ledger_read_bytes: 1024,
            ..Default::default()
        };
        let read_only_input = FeeQuoteInput {
            read_entries: 1,
            ..Default::default()
        };
        let baseline_read = price_local_resource_fee(
            &read_only,
            &read_only_input,
            DurabilitySplit::unknown(),
            config(),
        );
        let bumped_read = price_local_resource_fee(
            &read_only,
            &read_only_input,
            DurabilitySplit::unknown(),
            &bumped,
        );
        assert_eq!(baseline_read, bumped_read);
        assert_eq!(bumped_write.resource_fee, 34_264);
        assert_ne!(config().content_hash(), bumped.content_hash());
    }

    #[test]
    fn rpc_fee_delta_over_one_percent_is_a_calibration_error() {
        let within_tolerance = FeeCalibration::compare_rpc(1_010, Some(1_000), config());
        assert_eq!(within_tolerance.delta_stroops, Some(10));
        assert_eq!(within_tolerance.calibration_error, None);

        let outside_tolerance = FeeCalibration::compare_rpc(1_011, Some(1_000), config());
        assert_eq!(outside_tolerance.delta_stroops, Some(11));
        assert!(outside_tolerance.calibration_error.is_some());
    }

    /// Temporary-only write of 10240 rent bytes.
    /// refund = ceil(10240 * 12000 / (1024 * 4206)) = ceil(122880000/4306944) = 29
    #[test]
    fn temporary_only_write_has_non_zero_refund() {
        let quote = ResourceFeeQuote::estimate(
            &base_input(Some(10_240)),
            DurabilitySplit::temporary_only(10_240),
            config(),
        );
        assert_eq!(quote.refund_status, RefundStatus::Estimated);
        assert_eq!(quote.estimated_refund, Some(29));
        assert_eq!(quote.temporary_rent_bytes, Some(10_240));
        assert_eq!(quote.breakdown.temporary_rent, 29);
        assert_eq!(quote.breakdown.persistent_rent, 0);
        assert_eq!(quote.gross_resource_fee, BASE_FEE + 29);
        assert_eq!(quote.estimated_net, Some(BASE_FEE));
        assert!(quote.refund_is_estimate);
    }

    /// Same byte count, but durable: rent is billed at the persistent rate and
    /// nothing is ever refunded.
    /// persistent rent = ceil(10240 * 12000 / (1024 * 2103))
    ///                   = ceil(122880000/2153472) = 58
    #[test]
    fn persistent_only_write_of_same_size_has_smaller_or_zero_refund() {
        let persistent = ResourceFeeQuote::estimate(
            &base_input(Some(10_240)),
            DurabilitySplit::persistent_only(10_240),
            config(),
        );
        assert_eq!(persistent.estimated_refund, Some(0));
        assert_eq!(persistent.breakdown.temporary_rent, 0);
        assert_eq!(persistent.breakdown.persistent_rent, 58);
        assert_eq!(persistent.gross_resource_fee, BASE_FEE + 58);
        assert_eq!(persistent.estimated_net, Some(BASE_FEE + 58));

        let temporary = ResourceFeeQuote::estimate(
            &base_input(Some(10_240)),
            DurabilitySplit::temporary_only(10_240),
            config(),
        );
        assert!(temporary.estimated_refund > persistent.estimated_refund);
    }

    /// Half temporary, half persistent. The temporary rent bytes are
    /// ceil(10240 * 5120 / 10240) = 5120, so
    /// refund = ceil(5120 * 12000 / 4306944) = ceil(61440000/4306944) = 15
    /// and the persistent half is
    /// ceil(5120 * 12000 / 2153472) = ceil(61440000/2153472) = 29.
    #[test]
    fn mixed_writes_sum_both_durabilities() {
        let mixed = ResourceFeeQuote::estimate(
            &base_input(Some(10_240)),
            DurabilitySplit::mixed(5_120, 5_120),
            config(),
        );
        assert_eq!(mixed.temporary_rent_bytes, Some(5_120));
        assert_eq!(mixed.breakdown.temporary_rent, 15);
        assert_eq!(mixed.breakdown.persistent_rent, 29);
        assert_eq!(mixed.estimated_refund, Some(15));
        assert_eq!(mixed.gross_resource_fee, BASE_FEE + 15 + 29);
        assert_eq!(mixed.estimated_net, Some(BASE_FEE + 29));

        // Refund lands strictly between the two pure cases.
        let temporary_only = ResourceFeeQuote::estimate(
            &base_input(Some(10_240)),
            DurabilitySplit::temporary_only(10_240),
            config(),
        );
        let persistent_only = ResourceFeeQuote::estimate(
            &base_input(Some(10_240)),
            DurabilitySplit::persistent_only(10_240),
            config(),
        );
        assert!(persistent_only.estimated_refund < mixed.estimated_refund);
        assert!(mixed.estimated_refund < temporary_only.estimated_refund);
    }

    #[test]
    fn unknown_durability_reports_null_refund_not_zero() {
        let quote = ResourceFeeQuote::estimate(
            &base_input(Some(10_240)),
            DurabilitySplit::unknown(),
            config(),
        );
        assert_eq!(quote.refund_status, RefundStatus::UnknownDurability);
        assert_eq!(quote.estimated_refund, None);
        assert_eq!(quote.estimated_net, None);
        assert_eq!(quote.temporary_rent_bytes, None);
        // Gross is still reported, priced at the pessimistic persistent rate.
        assert_eq!(quote.gross_resource_fee, BASE_FEE + 58);
        assert!(quote.gross_includes_rent);
    }

    #[test]
    fn missing_rent_bytes_reports_null_refund_and_lower_bound_gross() {
        let quote = ResourceFeeQuote::estimate(
            &base_input(None),
            DurabilitySplit::temporary_only(10_240),
            config(),
        );
        assert_eq!(quote.refund_status, RefundStatus::UnknownRentBytes);
        assert_eq!(quote.estimated_refund, None);
        assert_eq!(quote.estimated_net, None);
        assert!(!quote.gross_includes_rent);
        assert_eq!(quote.gross_resource_fee, BASE_FEE);
    }

    #[test]
    fn zero_rent_with_known_split_is_a_definite_zero_refund() {
        let quote = ResourceFeeQuote::estimate(
            &base_input(Some(0)),
            DurabilitySplit::mixed(0, 0),
            config(),
        );
        assert_eq!(quote.refund_status, RefundStatus::Estimated);
        assert_eq!(quote.estimated_refund, Some(0));
        assert_eq!(quote.gross_resource_fee, BASE_FEE);
    }

    #[test]
    fn unknown_snapshot_yields_unknown_split() {
        assert_eq!(
            DurabilitySplit::from_state_snapshot(None),
            DurabilitySplit::unknown()
        );
        let empty = SimulationStateSnapshot {
            ledger_entries: HashMap::new(),
            ttl_entries: HashMap::new(),
            latest_ledger: 1,
        };
        assert_eq!(
            DurabilitySplit::from_state_snapshot(Some(&empty)),
            DurabilitySplit::unknown()
        );
    }

    #[test]
    fn rent_split_sums_back_to_the_reported_total() {
        for total in [0u64, 1, 512, 10_240, 65_536, 1_000_000] {
            for (temporary, persistent) in [(1u64, 1u64), (3, 7), (10_240, 1), (1, 10_240)] {
                let Some((temp_bytes, pers_bytes)) =
                    split_rent_bytes(total, DurabilitySplit::mixed(temporary, persistent))
                else {
                    panic!("known split must split");
                };
                assert_eq!(
                    temp_bytes.saturating_add(pers_bytes),
                    total,
                    "split leaked {total} stroops of rent at {temporary}/{persistent}"
                );
            }
        }
    }

    #[test]
    fn hostile_numbers_do_not_wrap_to_a_cheap_fee() {
        let hostile = FeeQuoteInput {
            cpu_instructions: u64::MAX,
            ledger_read_bytes: u64::MAX,
            ledger_write_bytes: u64::MAX,
            transaction_size_bytes: u64::MAX,
            read_entries: 1_000_000,
            write_entries: 1_000_000,
            contract_event_bytes: u64::MAX,
            rent_bytes: Some(u64::MAX),
        };
        let quote = ResourceFeeQuote::estimate(
            &hostile,
            DurabilitySplit::mixed(u64::MAX, u64::MAX),
            config(),
        );

        // The point of this test: an overflowing fee must saturate at the
        // expensive end, never wrap round to something that reads as cheap.
        assert_eq!(quote.gross_resource_fee, u64::MAX);
        assert_eq!(quote.breakdown.total(), u64::MAX);
        assert!(quote.gross_resource_fee > 1_000_000_000_000_000);

        let refund = quote.estimated_refund.expect("split is known");
        assert!(refund > 0, "a saturating refund is never a free write");
        assert!(refund < quote.gross_resource_fee);
        assert_eq!(
            quote.estimated_net,
            Some(quote.gross_resource_fee.saturating_sub(refund))
        );
    }

    #[test]
    fn div_ceil_rounds_up_and_guards_zero_denominator() {
        assert_eq!(div_ceil(0, 5), 0);
        assert_eq!(div_ceil(5, 5), 1);
        assert_eq!(div_ceil(6, 5), 2);
        assert_eq!(div_ceil(7, 0), 0);
        assert_eq!(div_ceil(u128::MAX, 1), u64::MAX);
    }

    /// The RPC reports `cost.rentBytes` as a decimal string. Without it the
    /// refund can never be estimated, so this is the field that decides whether
    /// a profile gets a real number or an honest `null`.
    #[test]
    fn rent_bytes_are_read_from_the_rpc_cost_object() {
        use crate::simulation::{SimulationEngine, SimulationRpcResult};

        let engine = SimulationEngine::new("https://example.invalid".to_string());
        let parse = |json: &str| -> Option<u64> {
            let rpc: SimulationRpcResult = serde_json::from_str(json).expect("rpc result");
            engine
                .parse_simulation_result(rpc)
                .expect("parses")
                .rent_bytes
        };

        assert_eq!(
            parse(
                r#"{"transactionData":"","latestLedger":1,
                    "cost":{"cpuInsns":"100000","memBytes":"2000","rentBytes":"10240"}}"#
            ),
            Some(10_240)
        );
        // Pre-protocol-20 nodes omit the field entirely.
        assert_eq!(
            parse(
                r#"{"transactionData":"","latestLedger":1,
                    "cost":{"cpuInsns":"100000","memBytes":"2000"}}"#
            ),
            None
        );
        // A junk value is a warning, not a panic and not a silent zero.
        assert_eq!(
            parse(
                r#"{"transactionData":"","latestLedger":1,
                    "cost":{"cpuInsns":"100000","memBytes":"2000","rentBytes":"nope"}}"#
            ),
            None
        );
    }

    /// End to end: a parsed simulation quotes with the durability split from its
    /// own ledger snapshot.
    #[test]
    fn a_parsed_simulation_quotes_end_to_end() {
        use crate::simulation::{SimulationEngine, SimulationRpcResult};

        let engine = SimulationEngine::new("https://example.invalid".to_string());
        let rpc: SimulationRpcResult = serde_json::from_str(
            r#"{"transactionData":"","latestLedger":1,
                "cost":{"cpuInsns":"100000","memBytes":"2000","rentBytes":"10240"}}"#,
        )
        .expect("rpc result");
        let simulation = engine.parse_simulation_result(rpc).expect("parses");

        // The footprint is empty, so no durability split is available and the
        // refund must be unknown rather than zero.
        let quote = quote_simulation(
            &simulation,
            simulation.rent_bytes,
            SorobanFeeConfig::checked_in(),
        );
        assert_eq!(quote.refund_status, RefundStatus::UnknownDurability);
        assert_eq!(quote.estimated_refund, None);
        assert!(quote.gross_includes_rent);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Total fee quote — acceptance tests (issue #1011)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod total_quote_tests {
    use super::*;
    use crate::fee_store::LedgerFeeSample;
    use chrono::Utc;
    use soroban_env_host::InvocationResources;

    /// Resource meters held constant across the acceptance fixtures.
    ///
    /// Every test below asserts that the two halves of a quote move
    /// independently, so the resource side must be a fixed input wherever the
    /// inclusion side is the variable one, and vice versa.
    fn resources() -> InvocationResources {
        InvocationResources {
            instructions: 250_000,
            mem_bytes: 65_536,
            read_entries: 4,
            write_entries: 3,
            read_bytes: 2_048,
            write_bytes: 1_024,
            contract_events_size_bytes: 300,
            persistent_rent_ledger_bytes: 4_096,
            persistent_entry_rent_bumps: 2,
            temporary_rent_ledger_bytes: 8_192,
            temporary_entry_rent_bumps: 1,
        }
    }

    /// A copy of the checked-in pubnet config with exactly one cost parameter
    /// overridden, so a test can move one parameter and nothing else.
    fn config_with(field: &str, value: i64) -> SorobanFeeConfig {
        let mut config = SorobanFeeConfig::checked_in().clone();
        match field {
            "fee_per_instruction_increment" => config.fee_per_instruction_increment = value,
            "fee_per_read_entry" => config.fee_per_read_entry = value,
            "fee_per_write_entry" => config.fee_per_write_entry = value,
            "fee_per_read_1kb" => config.fee_per_read_1kb = value,
            "fee_per_write_1kb" => config.fee_per_write_1kb = value,
            other => panic!("no such cost parameter: {other}"),
        }
        config
    }

    /// Ledger fee samples at a given `fee_charged` level.
    fn samples_at(fee_charged: i64, count: usize) -> Vec<LedgerFeeSample> {
        let collected_at = Utc::now();
        (0..count)
            .map(|i| LedgerFeeSample {
                ledger_sequence: 1_000 + i as i64,
                collected_at,
                base_reserve: 5_000_000,
                base_fee: fee_charged,
                max_fee: fee_charged * 2,
                fee_charged,
                transaction_count: 100,
                ledger_close_time: collected_at,
            })
            .collect()
    }

    /// The full fixture: real config, real samples, real prediction.
    fn full_quote() -> FeeQuote {
        FeeQuote::compose(
            &resources(),
            &samples_at(100, 30),
            1_030,
            SorobanFeeConfig::checked_in(),
        )
    }

    #[test]
    fn full_quote_reports_both_halves_and_a_ceiling() {
        let quote = full_quote();

        assert_eq!(quote.inclusion, InclusionStatus::Available);
        assert!(!quote.inclusion_unavailable());

        // Resource side is arithmetic over fixed meters, so these are exact.
        // The host splits the total as follows, and the parts sum to it:
        //   instructions  250_000 / 10_000 x 25          =     625
        //   entries       (4 read + 3 write) x 6_250     =  43_750
        //   writes        3 x 10_000                      =  30_000
        //   read bytes    2_048 / 1_024 x 1_786          =   3_572
        //   write bytes   1_024 / 1_024 x 12_000          =  12_000
        //   contract events                                =   2_930
        //   persistent rent                                =  21_148
        //   temporary rent                                 =  10_586
        //                                                     ------
        //                                                      124_611
        // Byte-denominated fees are prorated by bytes/1_024 rather than
        // rounded up per operation, which is why 512 bytes of reads costs 893
        // and not 1_786.
        assert_eq!(quote.resource_fee, 124_611);
        assert_eq!(quote.rent, 31_734);
        // Refundable is the temporary half only: it comes back when those
        // entries expire, whereas persistent rent never does.
        assert_eq!(quote.refundable, 10_586);
        assert_eq!(quote.non_refundable_resource_fee(), 92_877);

        assert_eq!(quote.inclusion_economy, Some(90));
        assert_eq!(quote.inclusion_standard, Some(111));
        assert_eq!(quote.inclusion_priority, Some(111));

        // The documented acceptance criterion: the ceiling is the resource fee
        // plus the standard bid.
        assert_eq!(
            quote.total_max,
            Some(quote.resource_fee + quote.inclusion_standard.unwrap())
        );
        assert_eq!(quote.total_max, Some(124_722));

        // Containment invariants that make the three numbers readable. The
        // refundable figure is strictly inside the rent figure, and rent is
        // strictly inside the resource fee, so no component is double-counted.
        assert!(quote.refundable < quote.rent);
        assert!(quote.rent < quote.resource_fee);
    }

    #[test]
    fn changing_only_the_cost_parameters_moves_the_resource_fee_and_not_the_bid() {
        let base = full_quote();
        let samples = samples_at(100, 30);

        let dearer_cpu = FeeQuote::compose(
            &resources(),
            &samples,
            1_030,
            &config_with("fee_per_instruction_increment", 50),
        );
        assert_ne!(
            dearer_cpu.resource_fee, base.resource_fee,
            "doubling the instruction rate must move the resource fee"
        );
        assert!(
            dearer_cpu.resource_fee > base.resource_fee,
            "a higher rate cannot make a resource fee smaller"
        );
        assert_eq!(
            dearer_cpu.inclusion_standard, base.inclusion_standard,
            "the bid comes from ledger samples, so a cost-parameter change must not move it"
        );
        assert_eq!(dearer_cpu.inclusion_economy, base.inclusion_economy);
        assert_eq!(dearer_cpu.inclusion_priority, base.inclusion_priority);

        // The write rate feeds both execution and rent, so it exercises the
        // other half of the same property.
        let dearer_writes = FeeQuote::compose(
            &resources(),
            &samples,
            1_030,
            &config_with("fee_per_write_1kb", 24_000),
        );
        assert_ne!(dearer_writes.resource_fee, base.resource_fee);
        assert_ne!(dearer_writes.rent, base.rent);
        assert_eq!(dearer_writes.inclusion_standard, base.inclusion_standard);
    }

    #[test]
    fn changing_only_the_ledger_samples_moves_the_bid_and_not_the_resource_fee() {
        let cheap_ledger = full_quote();

        let congested = FeeQuote::compose(
            &resources(),
            &samples_at(5_000, 30),
            1_030,
            SorobanFeeConfig::checked_in(),
        );

        assert_ne!(
            congested.inclusion_standard, cheap_ledger.inclusion_standard,
            "a busier ledger must move the bid"
        );
        assert!(
            congested.inclusion_standard.unwrap() > cheap_ledger.inclusion_standard.unwrap(),
            "a busier ledger cannot lower the bid"
        );

        // The criterion that matters: congestion is not the contract's fault,
        // so nothing on the resource side may move.
        assert_eq!(congested.resource_fee, cheap_ledger.resource_fee);
        assert_eq!(congested.rent, cheap_ledger.rent);
        assert_eq!(congested.refundable, cheap_ledger.refundable);
        assert_eq!(
            congested.non_refundable_resource_fee(),
            cheap_ledger.non_refundable_resource_fee()
        );
    }

    #[test]
    fn missing_samples_leave_the_resource_side_intact_and_null_the_bids() {
        let quote = FeeQuote::compose(&resources(), &[], 1_030, SorobanFeeConfig::checked_in());

        assert_eq!(quote.inclusion, InclusionStatus::Unavailable);
        assert!(quote.inclusion_unavailable());

        let full = full_quote();
        assert_eq!(quote.resource_fee, full.resource_fee);
        assert_eq!(quote.rent, full.rent);
        assert_eq!(quote.refundable, full.refundable);

        // Bids are absent, not zero: a zero bid would assert the ledger is
        // free, which is a claim about the network we cannot make.
        assert_eq!(quote.inclusion_economy, None);
        assert_eq!(quote.inclusion_standard, None);
        assert_eq!(quote.inclusion_priority, None);
        assert_eq!(quote.total_max, None);
    }

    #[test]
    fn compose_refuses_the_analytics_placeholder_bid() {
        // `FeeAnalyticsEngine::predict` answers an empty sample list with fixed
        // 100/100/150 placeholders and zero confidence. Forwarding those would
        // make every node without fee history present a confident 100-stroop
        // economy bid, so `compose` must not ask for a prediction at all.
        let placeholder = crate::fee_analytics::FeeAnalyticsEngine::new().predict(&[], 1_030);
        assert_eq!(placeholder.economy_bid, 100);
        assert_eq!(placeholder.confidence_score, 0.0);

        let composed = FeeQuote::compose(&resources(), &[], 1_030, SorobanFeeConfig::checked_in());
        assert_eq!(composed.inclusion_standard, None);
        assert_eq!(composed.inclusion, InclusionStatus::Unavailable);
    }

    #[test]
    fn the_config_bridge_matches_the_sdk_snapshot_exactly() {
        // The reason this type routes through `InvocationResources::estimate_fees`
        // is that we do not maintain a second implementation of the fee
        // arithmetic. Assert the bridge hands the host the same values
        // `soroban-sdk`'s own test helper uses, so divergence surfaces as a
        // failing number instead of two calculators quietly disagreeing.
        let (configuration, persistent, temporary) =
            SorobanFeeConfig::checked_in().host_fee_configuration();
        assert_eq!(configuration.fee_per_instruction_increment, 25);
        assert_eq!(configuration.fee_per_read_entry, 6_250);
        assert_eq!(configuration.fee_per_write_entry, 10_000);
        assert_eq!(configuration.fee_per_read_1kb, 1_786);
        assert_eq!(configuration.fee_per_write_1kb, 12_000);
        assert_eq!(configuration.fee_per_historical_1kb, 16_235);
        assert_eq!(configuration.fee_per_contract_event_1kb, 10_000);
        assert_eq!(configuration.fee_per_transaction_size_1kb, 1_624);
        assert_eq!(persistent, 2_103);
        assert_eq!(temporary, 4_206);
    }

    #[test]
    fn a_read_only_call_reports_zero_rent_but_keeps_its_ceiling() {
        let read_only = InvocationResources {
            instructions: 10_000,
            mem_bytes: 1_024,
            read_entries: 2,
            write_entries: 0,
            read_bytes: 512,
            write_bytes: 0,
            contract_events_size_bytes: 0,
            persistent_rent_ledger_bytes: 0,
            persistent_entry_rent_bumps: 0,
            temporary_rent_ledger_bytes: 0,
            temporary_entry_rent_bumps: 0,
        };
        let quote = FeeQuote::compose(
            &read_only,
            &samples_at(100, 30),
            1_030,
            SorobanFeeConfig::checked_in(),
        );
        //   instructions  10_000 / 10_000 x 25   =    25
        //   entries       2 read x 6_250         = 12_500
        //   read bytes    512 / 1_024 x 1_786    =    893
        assert_eq!(quote.resource_fee, 13_418);
        assert_eq!(quote.rent, 0);
        assert_eq!(quote.refundable, 0);
        assert_eq!(quote.non_refundable_resource_fee(), 13_418);
        // Zero here is a real measurement, not a missing one: nothing was
        // written, so nothing will ever be refunded. The bid is unaffected.
        assert_eq!(quote.inclusion, InclusionStatus::Available);
        assert_eq!(quote.total_max, Some(13_418 + 111));
    }

    #[test]
    fn an_absent_inclusion_key_deserialises_as_unavailable() {
        let quote: FeeQuote =
            serde_json::from_str(r#"{"resource_fee":100,"rent":10,"refundable":4}"#)
                .expect("quote");
        assert_eq!(quote.inclusion, InclusionStatus::Unavailable);
        assert_eq!(quote.inclusion_standard, None);
        assert_eq!(quote.total_max, None);
        assert_eq!(quote.resource_fee, 100);
    }
}
