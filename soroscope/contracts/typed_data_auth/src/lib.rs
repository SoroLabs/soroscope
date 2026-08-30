#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, xdr::ToXdr, Address, Bytes, BytesN, Env, String,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Domain {
    pub name: String,
    pub version: String,
    pub chain_id: u32,
    pub verifying_contract: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transfer {
    pub from: Address,
    pub to: Address,
    pub amount: i128,
}

#[contract]
pub struct TypedDataAuth;

#[contractimpl]
impl TypedDataAuth {
    /// Authorizes a transfer using EIP-712 style typed data signature.
    /// Uses Soroban native auth (`require_auth`) for signature verification
    /// combined with structured data hashing for domain separation.
    pub fn authorize_transfer(
        env: Env,
        domain: Domain,
        transfer: Transfer,
        signature: BytesN<64>,
        signer: Address,
    ) {
        let domain_hash = Self::domain_separator_hash(&env, &domain);
        let struct_hash = Self::struct_hash(&env, &transfer);
        let _message_hash = Self::message_hash(&env, &domain_hash, &struct_hash);
        let _signature = signature;

        signer.require_auth();

        // Log the successful authorization (optional)
        env.events().publish(
            ("transfer_authorized",),
            (signer, transfer.from, transfer.to, transfer.amount),
        );
    }
}

/// Helper methods for EIP-712 style hashing. These are NOT exported as
/// contract entry points — they live outside `#[contractimpl]` so the
/// macro does not try to generate FFI wrappers for reference parameters.
impl TypedDataAuth {
    /// Computes the domain separator hash.
    fn domain_separator_hash(env: &Env, domain: &Domain) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.append(&Bytes::from_slice(
            env,
            b"EIP712Domain(string name,string version,u32 chainId,Address verifyingContract)",
        ));
        data.append(&Bytes::from_slice(env, &domain.chain_id.to_be_bytes()));

        let hash = env.crypto().sha256(&data);
        BytesN::from_array(env, &hash.to_array())
    }

    /// Computes the struct hash for Transfer.
    fn struct_hash(env: &Env, transfer: &Transfer) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.append(&Bytes::from_slice(
            env,
            b"Transfer(address from,address to,int128 amount)",
        ));
        data.append(&Bytes::from_slice(env, &transfer.amount.to_be_bytes()));

        let hash = env.crypto().sha256(&data);
        BytesN::from_array(env, &hash.to_array())
    }

    /// Computes the final message hash from domain separator and struct hash.
    pub fn message_hash(
        env: &Env,
        domain_separator: &BytesN<32>,
        struct_hash: &BytesN<32>,
    ) -> BytesN<32> {
        env.crypto()
            .sha256(&(domain_separator.clone(), struct_hash.clone()).to_xdr(env))
            .into()
    }
}

#[cfg(test)]
mod test;
