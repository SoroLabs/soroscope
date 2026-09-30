//! Simulate a Stellar Asset Contract (SAC) `transfer`.
//!
//! Profiling `transfer` on its own contract id is awkward for a caller: you have
//! to know the SAC's contract id (which is *derived*, not published, for a
//! credit asset), and you have to hand the simulator both balances or the
//! simulation reads phantom entries off the live ledger. This module resolves
//! all of that and then hands off to the ordinary
//! [`SimulationEngine`](crate::simulation::SimulationEngine), so a SAC profile is
//! priced, footprinted and TTL-analysed exactly like any other contract call.
//!
//! # Contract id resolution
//!
//! A SAC is deployed by the `AssetRouter` at a deterministic id derived from the
//! network and the asset:
//!
//! ```text
//! network_id  = sha256(network_passphrase)
//! contract_id = sha256(xdr(HashIdPreimage::ContractId {
//!                 network_id,
//!                 contract_id_preimage: ContractIdPreimage::Asset(asset),
//!             }))[0..32]
//! ```
//!
//! so `USDC` + issuer on pubnet always lands on the same contract. The
//! derivation is pinned by a test against the published USDC SAC id.
//!
//! # Balances
//!
//! A SAC stores each holder's balance under `ScVal::Address(holder)` in
//! **persistent** contract data. A transfer reads the sender's and the
//! destination's balance, and writes both, so both entries must exist in the
//! simulation's footprint. When one is missing we refuse with
//! [`SacError::BalanceEntryMissing`], which names the account and which side it
//! was on — a missing destination balance is the single most common reason a
//! transfer simulation reports a wildly optimistic number (it silently reads a
//! zero balance and skips a write).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use stellar_strkey::Strkey;
use thiserror::Error;

use crate::simulation::{SimulationEngine, SimulationError, SimulationResult};

/// The `i128:` prefix understood by [`crate::parser::ArgParser`]. SAC amounts are
/// `i128`; without this a bare integer would parse as `i64` and the SAC would
/// reject the call with a type error that looks nothing like the real problem.
pub(crate) const I128_ARG_PREFIX: &str = "i128:";

/// Which side of a transfer a balance belongs to. Only used to make errors
/// actionable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BalanceRole {
    Sender,
    Destination,
}

impl BalanceRole {
    pub fn as_str(self) -> &'static str {
        match self {
            BalanceRole::Sender => "sender",
            BalanceRole::Destination => "destination",
        }
    }
}

impl fmt::Display for BalanceRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Error)]
pub enum SacError {
    #[error("invalid {role} account {account:?}: {reason}")]
    InvalidAccount {
        role: BalanceRole,
        account: String,
        reason: String,
    },

    #[error("invalid asset: {0}")]
    InvalidAsset(String),

    #[error(
        "no SAC balance entry for {role} account {account:?} on {contract_id}; \
         a SAC transfer reads and writes both balances, so the entry must exist \
         before the simulation is meaningful"
    )]
    BalanceEntryMissing {
        contract_id: String,
        account: String,
        role: BalanceRole,
    },

    #[error("invalid network passphrase: {0}")]
    InvalidNetwork(String),

    #[error("failed to encode SAC ledger entry: {0}")]
    Xdr(String),

    #[error(transparent)]
    Simulation(#[from] SimulationError),
}

/// How to name the asset being transferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SacAsset {
    /// A published SAC contract id (`C...`). Nothing is derived.
    ContractId(String),
    /// A credit asset, identified by code and issuer and resolved to its SAC.
    Credit { code: String, issuer: String },
    /// The native asset of the network.
    Native,
}

/// A resolved asset, for echoing back in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SacAssetDescriptor {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    pub contract_id: String,
}

impl SacAssetDescriptor {
    pub fn is_native(&self) -> bool {
        self.issuer.is_none()
    }
}

/// A request to profile a SAC transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SacTransferRequest {
    pub asset: SacAsset,
    /// `G...` account being debited.
    pub from: String,
    /// `G...` account being credited.
    pub to: String,
    /// Amount in the asset's smallest unit. Must be positive; a negative
    /// "transfer" is a burn or a bug, not a transfer, so it is rejected rather
    /// than priced.
    pub amount: i64,
    /// Network passphrase, used to derive the SAC contract id.
    pub network_passphrase: String,
    #[serde(default)]
    pub protocol_version: Option<u32>,
    #[serde(default)]
    pub enable_experimental: Option<bool>,
}

/// Balances to inject for the transfer. `None` means "look it up on the live
/// network"; see [`SacBalanceSource`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SacBalances {
    #[serde(default)]
    pub sender: Option<i128>,
    #[serde(default)]
    pub destination: Option<i128>,
}

/// Where a [`SacBalances`] came from. Injected balances keep a test hermetic;
/// live ones are the realistic case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SacBalanceSource {
    /// Balances supplied by the caller, not read from a node. Hermetic.
    Injected,
    /// Balances read from a live RPC.
    Live,
}

/// The `sac` block attached to a SAC transfer report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SacReport {
    pub contract_id: String,
    pub asset: SacAssetDescriptor,
    pub from: String,
    pub to: String,
    pub amount: i64,
    pub balance_source: SacBalanceSource,
}

/// A SAC transfer report: the ordinary simulation result, plus the SAC context
/// needed to make it interpretable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SacTransferReport {
    #[serde(flatten)]
    pub simulation: SimulationResult,
    pub sac: SacReport,
}

/// A SAC request after the contract id, addresses and balances are resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSacTransfer {
    pub contract_id: String,
    pub asset: SacAssetDescriptor,
    pub from: String,
    pub to: String,
    pub from_address: soroban_sdk::xdr::ScAddress,
    pub to_address: soroban_sdk::xdr::ScAddress,
    pub amount: i128,
    pub balances: SacBalances,
}

impl SacTransferRequest {
    /// Resolve the SAC contract id and both account addresses.
    ///
    /// This performs no I/O: given balances are used as-is. Callers that want
    /// live balances should fetch them first and pass them in, which also makes
    /// the whole path testable without a node.
    pub fn resolve(&self, balances: SacBalances) -> Result<ResolvedSacTransfer, SacError> {
        if self.amount <= 0 {
            return Err(SacError::InvalidAsset(format!(
                "transfer amount must be positive, got {}",
                self.amount
            )));
        }

        let from_address = parse_account(&self.from, BalanceRole::Sender)?;
        let to_address = parse_account(&self.to, BalanceRole::Destination)?;

        let (contract_id, asset) = self.resolve_asset()?;

        Ok(ResolvedSacTransfer {
            contract_id,
            asset,
            from: self.from.clone(),
            to: self.to.clone(),
            from_address,
            to_address,
            amount: self.amount as i128,
            balances,
        })
    }

    /// Resolve the SAC contract id and describe the asset.
    pub fn resolve_asset(&self) -> Result<(String, SacAssetDescriptor), SacError> {
        match &self.asset {
            SacAsset::ContractId(id) => Ok((
                id.clone(),
                SacAssetDescriptor {
                    code: "unknown".to_string(),
                    issuer: None,
                    contract_id: id.clone(),
                },
            )),
            SacAsset::Native => {
                let xdr_asset = soroban_sdk::xdr::Asset::Native;
                let id = sac_contract_id(&self.network_passphrase, &xdr_asset)?;
                Ok((
                    id.clone(),
                    SacAssetDescriptor {
                        code: "XLM".to_string(),
                        issuer: None,
                        contract_id: id,
                    },
                ))
            }
            SacAsset::Credit { code, issuer } => {
                let xdr_asset = credit_asset(code, issuer)?;
                let id = sac_contract_id(&self.network_passphrase, &xdr_asset)?;
                Ok((
                    id.clone(),
                    SacAssetDescriptor {
                        code: code.clone(),
                        issuer: Some(issuer.clone()),
                        contract_id: id,
                    },
                ))
            }
        }
    }
}

fn parse_account(
    account: &str,
    role: BalanceRole,
) -> Result<soroban_sdk::xdr::ScAddress, SacError> {
    crate::parser::ArgParser::parse_address(account).map_err(|e| SacError::InvalidAccount {
        role,
        account: account.to_string(),
        reason: e.to_string(),
    })
}

/// Build the XDR `Asset` for a credit asset, choosing the 4- or 12-character
/// code variant.
fn credit_asset(code: &str, issuer: &str) -> Result<soroban_sdk::xdr::Asset, SacError> {
    use soroban_sdk::xdr::{AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4};
    use soroban_sdk::xdr::{PublicKey, Uint256};

    let account_id = Strkey::from_string(issuer)
        .ok()
        .and_then(|s| match s {
            Strkey::PublicKeyEd25519(pk) => Some(pk),
            _ => None,
        })
        .map(|pk| AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(pk.0))))
        .ok_or_else(|| {
            SacError::InvalidAsset(format!("issuer {issuer:?} is not a G... account id"))
        })?;

    match code.len() {
        4 => {
            let asset_code: AssetCode4 = code
                .as_bytes()
                .try_into()
                .map_err(|_| SacError::InvalidAsset("asset code must be 4 ASCII bytes".into()))?;
            Ok(Asset::CreditAlphanum4(AlphaNum4 {
                asset_code,
                issuer: account_id,
            }))
        }
        12 => {
            let asset_code: AssetCode12 = code
                .as_bytes()
                .try_into()
                .map_err(|_| SacError::InvalidAsset("asset code must be 12 ASCII bytes".into()))?;
            Ok(Asset::CreditAlphanum12(AlphaNum12 {
                asset_code,
                issuer: account_id,
            }))
        }
        other => Err(SacError::InvalidAsset(format!(
            "asset code must be 4 or 12 characters, got {other}"
        ))),
    }
}

/// Derive the SAC contract id for `asset` on the network identified by
/// `network_passphrase`.
fn sac_contract_id(
    network_passphrase: &str,
    asset: &soroban_sdk::xdr::Asset,
) -> Result<String, SacError> {
    use soroban_sdk::xdr::{
        ContractIdPreimage, Hash, HashIdPreimage, HashIdPreimageContractId, Limits, WriteXdr,
    };

    if network_passphrase.trim().is_empty() {
        return Err(SacError::InvalidNetwork(
            "network passphrase is empty; a SAC contract id cannot be derived without it".into(),
        ));
    }

    let network_id = Hash(sha256(network_passphrase.as_bytes()));
    let preimage = HashIdPreimage::ContractId(HashIdPreimageContractId {
        network_id,
        contract_id_preimage: ContractIdPreimage::Asset(asset.clone()),
    });
    let encoded = preimage
        .to_xdr(Limits::none())
        .map_err(|e| SacError::Xdr(format!("failed to encode SAC contract id preimage: {e}")))?;

    let hash = sha256(&encoded);
    Ok(Strkey::Contract(stellar_strkey::Contract(hash)).to_string())
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn i128_to_sc_val(value: i128) -> soroban_sdk::xdr::ScVal {
    soroban_sdk::xdr::ScVal::I128(soroban_sdk::xdr::Int128Parts {
        hi: (value >> 64) as i64,
        lo: value as u64,
    })
}

impl ResolvedSacTransfer {
    /// The SAC contract instance ledger key.
    fn instance_key(&self) -> Result<soroban_sdk::xdr::LedgerKey, SacError> {
        use soroban_sdk::xdr::{LedgerKey, LedgerKeyContractData, ScVal};
        Ok(LedgerKey::ContractData(LedgerKeyContractData {
            contract: self.contract_address()?,
            key: ScVal::LedgerKeyContractInstance,
            durability: soroban_sdk::xdr::ContractDataDurability::Persistent,
        }))
    }

    fn contract_address(&self) -> Result<soroban_sdk::xdr::ScAddress, SacError> {
        parse_account(&self.contract_id, BalanceRole::Sender).map_err(|_| {
            SacError::InvalidAsset(format!(
                "resolved SAC contract id {:?} is not a C... address",
                self.contract_id
            ))
        })
    }

    /// The SAC balance ledger key for one account. SAC stores balances under
    /// `ScVal::Address(holder)` in persistent contract data.
    fn balance_key(
        &self,
        account: &soroban_sdk::xdr::ScAddress,
    ) -> Result<soroban_sdk::xdr::LedgerKey, SacError> {
        use soroban_sdk::xdr::{LedgerKey, LedgerKeyContractData, ScVal};
        Ok(LedgerKey::ContractData(LedgerKeyContractData {
            contract: self.contract_address()?,
            key: ScVal::Address(account.clone()),
            durability: soroban_sdk::xdr::ContractDataDurability::Persistent,
        }))
    }

    fn instance_entry(&self) -> Result<soroban_sdk::xdr::LedgerEntry, SacError> {
        use soroban_sdk::xdr::{
            ContractDataEntry, ContractExecutable, ExtensionPoint, LedgerEntry, LedgerEntryData,
            LedgerEntryExt, ScContractInstance, ScVal,
        };
        let contract = self.contract_address()?;
        Ok(LedgerEntry {
            last_modified_ledger_seq: 0,
            data: LedgerEntryData::ContractData(ContractDataEntry {
                ext: ExtensionPoint::V0,
                contract: contract.clone(),
                key: ScVal::LedgerKeyContractInstance,
                durability: soroban_sdk::xdr::ContractDataDurability::Persistent,
                // `StellarAsset` is what tells the host this instance is a SAC
                // rather than a Wasm contract, so the injected entry makes the
                // simulation resolve the right code path.
                val: ScVal::ContractInstance(ScContractInstance {
                    executable: ContractExecutable::StellarAsset,
                    storage: None,
                }),
            }),
            ext: LedgerEntryExt::V0,
        })
    }

    fn balance_entry(
        &self,
        account: &soroban_sdk::xdr::ScAddress,
        balance: i128,
    ) -> Result<soroban_sdk::xdr::LedgerEntry, SacError> {
        use soroban_sdk::xdr::{
            ContractDataEntry, ExtensionPoint, LedgerEntry, LedgerEntryData, LedgerEntryExt, ScVal,
        };
        let contract = self.contract_address()?;
        Ok(LedgerEntry {
            last_modified_ledger_seq: 0,
            data: LedgerEntryData::ContractData(ContractDataEntry {
                ext: ExtensionPoint::V0,
                contract: contract.clone(),
                key: ScVal::Address(account.clone()),
                durability: soroban_sdk::xdr::ContractDataDurability::Persistent,
                val: i128_to_sc_val(balance),
            }),
            ext: LedgerEntryExt::V0,
        })
    }

    /// Build the ledger overrides a SAC transfer needs: the contract instance
    /// plus both balance entries.
    ///
    /// Fails with [`SacError::BalanceEntryMissing`] naming the account if either
    /// balance is absent — a transfer cannot be priced against a balance that
    /// was never read.
    pub fn build_ledger_overrides(&self) -> Result<HashMap<String, String>, SacError> {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        use soroban_sdk::xdr::{Limits, WriteXdr};

        let sender_balance = self
            .balances
            .sender
            .ok_or_else(|| SacError::BalanceEntryMissing {
                contract_id: self.contract_id.clone(),
                account: self.from.clone(),
                role: BalanceRole::Sender,
            })?;
        let destination_balance =
            self.balances
                .destination
                .ok_or_else(|| SacError::BalanceEntryMissing {
                    contract_id: self.contract_id.clone(),
                    account: self.to.clone(),
                    role: BalanceRole::Destination,
                })?;

        let entries = [
            (self.instance_key()?, self.instance_entry()?),
            (
                self.balance_key(&self.from_address)?,
                self.balance_entry(&self.from_address, sender_balance)?,
            ),
            (
                self.balance_key(&self.to_address)?,
                self.balance_entry(&self.to_address, destination_balance)?,
            ),
        ];

        let mut overrides = HashMap::with_capacity(entries.len());
        for (key, entry) in entries {
            let key_b64 = BASE64.encode(
                key.to_xdr(Limits::none())
                    .map_err(|e| SacError::Xdr(format!("failed to encode SAC ledger key: {e}")))?,
            );
            let entry_b64 =
                BASE64.encode(entry.to_xdr(Limits::none()).map_err(|e| {
                    SacError::Xdr(format!("failed to encode SAC ledger entry: {e}"))
                })?);
            overrides.insert(key_b64, entry_b64);
        }
        Ok(overrides)
    }

    /// Argument strings for `transfer(to, amount)`, in the form the engine's
    /// generic argument parser accepts.
    pub fn transfer_args(&self) -> Vec<String> {
        vec![self.to.clone(), format!("{I128_ARG_PREFIX}{}", self.amount)]
    }

    /// The `sac` block for the report.
    pub fn report(&self, balance_source: SacBalanceSource) -> SacReport {
        SacReport {
            contract_id: self.contract_id.clone(),
            asset: self.asset.clone(),
            from: self.from.clone(),
            to: self.to.clone(),
            amount: self.amount as i64,
            balance_source,
        }
    }
}

/// Resolve `request` and profile the transfer with `engine`.
///
/// Reuses [`SimulationEngine::simulate_from_contract_id`] with the SAC instance
/// and both balances injected, so the result is priced and TTL-analysed by
/// exactly the same code path as an ordinary contract call.
pub async fn simulate_sac_transfer(
    engine: &SimulationEngine,
    request: &SacTransferRequest,
    balances: SacBalances,
    balance_source: SacBalanceSource,
) -> Result<SacTransferReport, SacError> {
    let resolved = request.resolve(balances)?;
    let overrides = resolved.build_ledger_overrides()?;

    tracing::debug!(
        contract_id = %resolved.contract_id,
        from = %resolved.from,
        to = %resolved.to,
        amount = resolved.amount,
        overrides = overrides.len(),
        "simulating SAC transfer"
    );

    let simulation = engine
        .simulate_from_contract_id(
            &resolved.contract_id,
            "transfer",
            resolved.transfer_args(),
            Some(overrides),
            request.protocol_version,
            request.enable_experimental,
        )
        .await?;

    Ok(SacTransferReport {
        simulation,
        sac: resolved.report(balance_source),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{SimulationResult, SimulationStateSnapshot};
    use soroban_sdk::xdr::{Limits, ReadXdr, ScVal};

    const PUBNET: &str = "Public Global Stellar Network ; September 2015";
    const TESTNET: &str = "Test SDF Network ; September 2015";
    const FUTURENET: &str = "Test SDF Future Network ; October 2022";

    const USDC_ISSUER: &str = "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";
    /// The published USDC SAC on pubnet.
    const USDC_SAC_PUBNET: &str = "CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75";

    const ALICE: &str = "GAAQQDYWDUSCWMRZIBDU4VK4MNVHC6D7Q2GZJG5CVGYLPPWFZTJ5V6UJ";
    const BOB: &str = "GABASEAXDYSSYMZ2IFEE6VS5MRVXE6MAQ6HJLHFDVKY3RP6GZXKNWWW3";

    fn usdc_request() -> SacTransferRequest {
        SacTransferRequest {
            asset: SacAsset::Credit {
                code: "USDC".to_string(),
                issuer: USDC_ISSUER.to_string(),
            },
            from: ALICE.to_string(),
            to: BOB.to_string(),
            amount: 1_000_000,
            network_passphrase: PUBNET.to_string(),
            protocol_version: Some(22),
            enable_experimental: None,
        }
    }

    fn balances() -> SacBalances {
        SacBalances {
            sender: Some(5_000_000),
            destination: Some(0),
        }
    }

    /// Pins the contract-id derivation against the published USDC SAC. If the
    /// preimage shape or the network-id derivation ever changes, this fails
    /// rather than silently profiling the wrong contract.
    #[test]
    fn derives_the_published_usdc_sac_contract_id() {
        let asset = credit_asset("USDC", USDC_ISSUER).expect("USDC asset");
        let id = sac_contract_id(PUBNET, &asset).expect("contract id");
        assert_eq!(id, USDC_SAC_PUBNET);
    }

    /// The native asset resolves to a different contract on every network, and
    /// to the same one on repeat calls.
    #[test]
    fn native_asset_derives_per_network() {
        let native = soroban_sdk::xdr::Asset::Native;
        let pubnet = sac_contract_id(PUBNET, &native).unwrap();
        let testnet = sac_contract_id(TESTNET, &native).unwrap();
        let futurenet = sac_contract_id(FUTURENET, &native).unwrap();

        assert!(pubnet.starts_with('C'));
        assert_ne!(pubnet, testnet);
        assert_ne!(pubnet, futurenet);
        assert_ne!(testnet, futurenet);
        assert_eq!(pubnet, sac_contract_id(PUBNET, &native).unwrap());
    }

    #[test]
    fn contract_id_depends_on_the_network() {
        let asset = credit_asset("USDC", USDC_ISSUER).unwrap();
        let pubnet = sac_contract_id(PUBNET, &asset).unwrap();
        let futurenet = sac_contract_id(FUTURENET, &asset).unwrap();
        assert_ne!(pubnet, futurenet);
        assert_eq!(pubnet, USDC_SAC_PUBNET);
    }

    #[test]
    fn native_asset_resolves_on_pubnet() {
        let request = SacTransferRequest {
            asset: SacAsset::Native,
            ..usdc_request()
        };
        let (id, descriptor) = request.resolve_asset().unwrap();
        assert!(id.starts_with('C'));
        assert_eq!(descriptor.code, "XLM");
        assert!(descriptor.is_native());
        assert_eq!(descriptor.contract_id, id);
        assert_eq!(
            id,
            sac_contract_id(PUBNET, &soroban_sdk::xdr::Asset::Native).unwrap()
        );
    }

    #[test]
    fn explicit_contract_id_is_passed_through() {
        let request = SacTransferRequest {
            asset: SacAsset::ContractId(USDC_SAC_PUBNET.to_string()),
            ..usdc_request()
        };
        let resolved = request.resolve(balances()).unwrap();
        assert_eq!(resolved.contract_id, USDC_SAC_PUBNET);
        assert_eq!(resolved.amount, 1_000_000);
    }

    #[test]
    fn rejects_a_non_positive_amount() {
        for amount in [0, -1, i64::MIN] {
            let request = SacTransferRequest {
                amount,
                ..usdc_request()
            };
            let err = request.resolve(balances()).unwrap_err();
            assert!(
                matches!(&err, SacError::InvalidAsset(m) if m.contains("must be positive")),
                "amount {amount} gave {err}"
            );
        }
    }

    #[test]
    fn rejects_a_malformed_sender_naming_the_account() {
        let request = SacTransferRequest {
            from: "not-an-account".to_string(),
            ..usdc_request()
        };
        let err = request.resolve(balances()).unwrap_err();
        match err {
            SacError::InvalidAccount { role, account, .. } => {
                assert_eq!(role, BalanceRole::Sender);
                assert_eq!(account, "not-an-account");
            }
            other => panic!("expected InvalidAccount, got {other}"),
        }
    }

    #[test]
    fn rejects_a_malformed_destination_naming_the_account() {
        let request = SacTransferRequest {
            to: "nope".to_string(),
            ..usdc_request()
        };
        let err = request.resolve(balances()).unwrap_err();
        match err {
            SacError::InvalidAccount { role, account, .. } => {
                assert_eq!(role, BalanceRole::Destination);
                assert_eq!(account, "nope");
            }
            other => panic!("expected InvalidAccount, got {other}"),
        }
    }

    #[test]
    fn missing_sender_balance_names_the_sender() {
        let request = usdc_request();
        let err = request
            .resolve(SacBalances {
                sender: None,
                destination: Some(0),
            })
            .and_then(|r| r.build_ledger_overrides())
            .unwrap_err();
        match err {
            SacError::BalanceEntryMissing { account, role, .. } => {
                assert_eq!(account, ALICE);
                assert_eq!(role, BalanceRole::Sender);
            }
            other => panic!("expected BalanceEntryMissing, got {other}"),
        }
    }

    #[test]
    fn missing_destination_balance_names_the_destination() {
        let request = usdc_request();
        let err = request
            .resolve(SacBalances {
                sender: Some(1),
                destination: None,
            })
            .and_then(|r| r.build_ledger_overrides())
            .unwrap_err();
        match err {
            SacError::BalanceEntryMissing { account, role, .. } => {
                assert_eq!(account, BOB);
                assert_eq!(role, BalanceRole::Destination);
            }
            other => panic!("expected BalanceEntryMissing, got {other}"),
        }
    }

    /// The hermetic case: injected balances, no node contacted. Asserts the
    /// override set is exactly the instance plus both balances, and that it
    /// survives the engine's own base64/XDR round-trip.
    #[test]
    fn injected_entries_are_well_formed_and_round_trip() {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        use soroban_sdk::xdr::{
            ContractDataDurability, ContractExecutable, LedgerEntry, LedgerKey, ScVal,
        };

        let resolved = usdc_request().resolve(balances()).unwrap();
        let overrides = resolved.build_ledger_overrides().unwrap();
        assert_eq!(overrides.len(), 3, "instance + sender + destination");

        let mut instance = 0;
        let mut balances_seen = Vec::new();
        for (key_b64, entry_b64) in &overrides {
            let key = LedgerKey::from_xdr(&BASE64.decode(key_b64).unwrap(), Limits::none())
                .expect("key decodes");
            let entry = LedgerEntry::from_xdr(&BASE64.decode(entry_b64).unwrap(), Limits::none())
                .expect("entry decodes");

            let soroban_sdk::xdr::LedgerKey::ContractData(data) = &key else {
                panic!("all SAC entries are contract data");
            };
            assert_eq!(data.durability, ContractDataDurability::Persistent);

            match &data.key {
                ScVal::LedgerKeyContractInstance => {
                    instance += 1;
                    let soroban_sdk::xdr::LedgerEntryData::ContractData(entry_data) = &entry.data
                    else {
                        panic!("instance entry is contract data");
                    };
                    let ScVal::ContractInstance(instance) = &entry_data.val else {
                        panic!("instance entry value is a contract instance");
                    };
                    assert_eq!(
                        instance.executable,
                        ContractExecutable::StellarAsset,
                        "a SAC instance is not a Wasm contract"
                    );
                }
                ScVal::Address(holder) => {
                    balances_seen.push(holder.clone());
                    let soroban_sdk::xdr::LedgerEntryData::ContractData(entry_data) = &entry.data
                    else {
                        panic!("balance entry is contract data");
                    };
                    let ScVal::I128(balance) = &entry_data.val else {
                        panic!("balance entry value is an i128");
                    };
                    let value = ((balance.hi as i128) << 64) | balance.lo as i128;
                    assert!(value >= 0, "injected balance must be non-negative");
                }
                other => panic!("unexpected SAC storage key: {other:?}"),
            }
        }

        assert_eq!(instance, 1, "exactly one contract instance entry");
        assert_eq!(balances_seen.len(), 2, "exactly two balance entries");
        assert_ne!(balances_seen[0], balances_seen[1], "distinct holders");
    }

    #[test]
    fn balance_entry_key_is_the_holder_address() {
        let resolved = usdc_request().resolve(balances()).unwrap();
        let key = resolved.balance_key(&resolved.to_address).unwrap();
        let soroban_sdk::xdr::LedgerKey::ContractData(data) = key else {
            panic!("balance key is contract data");
        };
        assert_eq!(
            data.key,
            ScVal::Address(resolved.to_address.clone()),
            "SAC keys balances by holder address"
        );
    }

    #[test]
    fn transfer_args_encode_the_amount_as_i128() {
        let args = usdc_request().resolve(balances()).unwrap().transfer_args();
        assert_eq!(args[0], BOB);
        assert_eq!(args[1], "i128:1000000");
    }

    #[test]
    fn report_carries_the_sac_block_beside_the_simulation() {
        let resolved = usdc_request().resolve(balances()).unwrap();
        let report = SacTransferReport {
            simulation: SimulationResult {
                bytes_by_durability: crate::simulation::BytesByDurability::default(),
                resources: Default::default(),
                transaction_hash: None,
                latest_ledger: 42,
                cost_stroops: 1234,
                rent_bytes: None,
                state_dependency: None,
                ttl_analysis: None,
                transaction_data: String::new(),
                call_graph: None,
                state_snapshot: Some(SimulationStateSnapshot {
                    ledger_entries: HashMap::new(),
                    ttl_entries: HashMap::new(),
                    latest_ledger: 42,
                }),
                protocol_version: 22,
                cost_breakdown: None,
                ..Default::default()
            },
            sac: resolved.report(SacBalanceSource::Injected),
        };

        // The simulation result is flattened, not nested, so existing
        // consumers keep working and `sac` is simply an extra key.
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["cost_stroops"], 1234);
        assert_eq!(json["latest_ledger"], 42);
        assert_eq!(json["sac"]["contract_id"], USDC_SAC_PUBNET);
        assert_eq!(json["sac"]["asset"]["code"], "USDC");
        assert_eq!(json["sac"]["asset"]["issuer"], USDC_ISSUER);
        assert_eq!(json["sac"]["from"], ALICE);
        assert_eq!(json["sac"]["to"], BOB);
        assert_eq!(json["sac"]["amount"], 1_000_000);
        assert_eq!(json["sac"]["balance_source"], "injected");
    }

    #[test]
    fn credit_asset_rejects_a_bad_issuer_and_code_length() {
        assert!(credit_asset("USDC", "GNOTREAL").is_err());
        assert!(credit_asset("TOOLONGCODE", USDC_ISSUER).is_err());
        assert!(credit_asset("US", USDC_ISSUER).is_err());
        assert!(credit_asset("USDC", USDC_ISSUER).is_ok());
    }

    #[test]
    fn empty_network_passphrase_is_rejected() {
        let request = SacTransferRequest {
            network_passphrase: "   ".to_string(),
            ..usdc_request()
        };
        let err = request.resolve(balances()).unwrap_err();
        assert!(matches!(err, SacError::InvalidNetwork(_)), "got {err}");
    }

    /// Live end-to-end. `cargo test` skips it; run explicitly with
    /// `SOROSCOPE_LIVE_RPC=<url> cargo test -p soroscope-core --lib -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "requires a live RPC; set SOROSCOPE_LIVE_RPC to run"]
    async fn live_sac_transfer_on_a_public_network() {
        let Ok(rpc_url) = std::env::var("SOROSCOPE_LIVE_RPC") else {
            panic!("set SOROSCOPE_LIVE_RPC to a public RPC url to run this test");
        };

        let request = SacTransferRequest {
            asset: SacAsset::Native,
            from: ALICE.to_string(),
            to: BOB.to_string(),
            amount: 1,
            network_passphrase: PUBNET.to_string(),
            protocol_version: Some(22),
            enable_experimental: None,
        };

        let engine = SimulationEngine::new(rpc_url);

        let report = simulate_sac_transfer(
            &engine,
            &request,
            SacBalances {
                sender: Some(0),
                destination: Some(0),
            },
            SacBalanceSource::Live,
        )
        .await
        .expect("live SAC transfer simulation");

        assert!(report.sac.contract_id.starts_with('C'));
        assert!(report.sac.asset.is_native());
        assert!(!report.sac.contract_id.is_empty());
    }
}
