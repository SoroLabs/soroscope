#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Env, Map, String, Vec,
};

#[cfg(test)]
mod test;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    EmptyBatch = 1,
    LengthMismatch = 2,
    InvalidAmount = 3,
    InsufficientBalance = 4,
    TooManyRecipients = 5,
}

/// Upper bound on recipients / transfers per batch. Oversized batches are
/// rejected before any iteration so a caller cannot exhaust the
/// transaction's CPU instruction budget by passing an unbounded batch.
const MAX_RECIPIENTS: u32 = 100;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionMode {
    AllOrNothing,
    Partial,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransferFailure {
    None,
    InvalidAmount,
    InsufficientBalance,
}

/// Specification of an individual transfer within a batch.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transfer {
    pub from: Address,
    pub to: Address,
    pub amount: i128,
}

/// Result of executing or quoting an individual transfer in the batch.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferResult {
    pub recipient: Address,
    pub amount: i128,
    pub success: bool,
    pub failure: TransferFailure,
}

#[soroban_sdk::contractclient(name = "BatchTokenClient")]
pub trait BatchToken {
    fn balance(e: Env, id: Address) -> i128;
    fn transfer(e: Env, from: Address, to: Address, amount: i128);
}

fn validate_batch_size(len: u32) -> Result<(), Error> {
    if len == 0 {
        return Err(Error::EmptyBatch);
    }
    if len > MAX_RECIPIENTS {
        return Err(Error::TooManyRecipients);
    }
    Ok(())
}

fn process_batch(
    env: &Env,
    token: &Address,
    transfers: &Vec<Transfer>,
    mode: &ExecutionMode,
    do_transfer: bool,
) -> Result<Vec<TransferResult>, Error> {
    validate_batch_size(transfers.len())?;

    let token_client = BatchTokenClient::new(env, token);
    let mut remaining_balances: Map<Address, i128> = Map::new(env);
    let mut results = Vec::new(env);

    let zero_address_g = Address::from_string(&String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ));
    let zero_address_c = Address::from_string(&String::from_str(
        env,
        "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM",
    ));

    let is_all_or_nothing = matches!(mode, ExecutionMode::AllOrNothing);

    for transfer in transfers.iter() {
        let recipient = transfer.to;
        let amount = transfer.amount;
        let from = transfer.from;

        if amount <= 0
            || recipient == zero_address_g
            || recipient == zero_address_c
            || from == zero_address_g
            || from == zero_address_c
        {
            if is_all_or_nothing {
                return Err(Error::InvalidAmount);
            }
            results.push_back(TransferResult {
                recipient,
                amount,
                success: false,
                failure: TransferFailure::InvalidAmount,
            });
            continue;
        }

        let sender_balance = if let Some(bal) = remaining_balances.get(from.clone()) {
            bal
        } else {
            let bal = token_client.balance(&from);
            remaining_balances.set(from.clone(), bal);
            bal
        };

        if sender_balance < amount {
            if is_all_or_nothing {
                return Err(Error::InsufficientBalance);
            }
            results.push_back(TransferResult {
                recipient,
                amount,
                success: false,
                failure: TransferFailure::InsufficientBalance,
            });
            continue;
        }

        remaining_balances.set(from.clone(), sender_balance - amount);

        if do_transfer {
            token_client.transfer(&from, &recipient, &amount);
        }

        results.push_back(TransferResult {
            recipient,
            amount,
            success: true,
            failure: TransferFailure::None,
        });
    }

    Ok(results)
}

#[contract]
pub struct BatchTransfer;

#[contractimpl]
impl BatchTransfer {
    /// Executes a batch of transfers after verifying per-transfer authorization.
    ///
    /// For each transfer in the batch, `transfer.from.require_auth()` is enforced
    /// to ensure that every transfer's sender has explicitly authorized the transfer.
    pub fn execute(
        env: Env,
        token: Address,
        transfers: Vec<Transfer>,
        mode: ExecutionMode,
    ) -> Result<Vec<TransferResult>, Error> {
        validate_batch_size(transfers.len())?;

        let mut authenticated_senders: Vec<Address> = Vec::new(&env);
        for transfer in transfers.iter() {
            if !authenticated_senders.contains(&transfer.from) {
                transfer.from.require_auth();
                authenticated_senders.push_back(transfer.from.clone());
            }
        }

        process_batch(&env, &token, &transfers, &mode, true)
    }

    /// Simulates/quotes a batch of transfers without executing them on-chain.
    pub fn quote(
        env: Env,
        token: Address,
        transfers: Vec<Transfer>,
        mode: ExecutionMode,
    ) -> Result<Vec<TransferResult>, Error> {
        process_batch(&env, &token, &transfers, &mode, false)
    }

    /// Executes transfers from a single sender to multiple recipients.
    /// Verifies `sender.require_auth()` before processing.
    pub fn execute_single_sender(
        env: Env,
        token: Address,
        sender: Address,
        recipients: Vec<Address>,
        amounts: Vec<i128>,
        mode: ExecutionMode,
    ) -> Result<Vec<TransferResult>, Error> {
        sender.require_auth();
        if recipients.len() != amounts.len() {
            return Err(Error::LengthMismatch);
        }
        let mut transfers = Vec::new(&env);
        for (recipient, amount) in recipients.iter().zip(amounts.iter()) {
            transfers.push_back(Transfer {
                from: sender.clone(),
                to: recipient,
                amount,
            });
        }
        process_batch(&env, &token, &transfers, &mode, true)
    }
}
