#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String, Vec};

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

/// Upper bound on recipients per batch. Oversized recipient vectors are
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

fn validate_lengths(recipients: &Vec<Address>, amounts: &Vec<i128>) -> Result<u32, Error> {
    let len = recipients.len();
    if len == 0 {
        return Err(Error::EmptyBatch);
    }
    if len > MAX_RECIPIENTS {
        return Err(Error::TooManyRecipients);
    }
    if len != amounts.len() {
        return Err(Error::LengthMismatch);
    }
    Ok(len)
}
fn simulate_batch(
    env: &Env,
    token: &Address,
    sender: &Address,
    recipients: &Vec<Address>,
    amounts: &Vec<i128>,
    mode: &ExecutionMode,
) -> Result<Vec<TransferResult>, Error> {
    let len = validate_lengths(recipients, amounts)?;
    let token_client = BatchTokenClient::new(env, token);
    let mut remaining_balance = token_client.balance(sender);
    let mut results = Vec::new(env);

    let zero_address_g = Address::from_string(&String::from_str(env, "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"));
    let zero_address_c = Address::from_string(&String::from_str(env, "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABU"));

    for i in 0..len {
        let recipient = recipients.get(i).unwrap();
        let amount = amounts.get(i).unwrap();

        if amount <= 0 || recipient == zero_address_g || recipient == zero_address_c {
            if matches!(mode, ExecutionMode::AllOrNothing) {
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

        if remaining_balance < amount {
            if matches!(mode, ExecutionMode::AllOrNothing) {
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

        remaining_balance -= amount;
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
    pub fn execute(
        env: Env,
        token: Address,
        sender: Address,
        recipients: Vec<Address>,
        amounts: Vec<i128>,
        mode: ExecutionMode,
    ) -> Result<Vec<TransferResult>, Error> {
        sender.require_auth();

        let plan = simulate_batch(&env, &token, &sender, &recipients, &amounts, &mode)?;
        let token_client = BatchTokenClient::new(&env, &token);

        for item in plan.iter() {
            if item.success {
                token_client.transfer(&sender, &item.recipient, &item.amount);
            }
        }

        Ok(plan)
    }

    pub fn quote(
        env: Env,
        token: Address,
        sender: Address,
        recipients: Vec<Address>,
        amounts: Vec<i128>,
        mode: ExecutionMode,
    ) -> Result<Vec<TransferResult>, Error> {
        simulate_batch(&env, &token, &sender, &recipients, &amounts, &mode)
    }
}
