#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, xdr::ToXdr, Address, Bytes, BytesN, Env,
    String,
};

mod types;
mod errors;
mod constants;
mod test;

use types::{Gift, GiftStatus};
use errors::Error;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Gift(u64),
    NextGiftId,
    Oracle, // Stores BytesN<32> (Ed25519 Public Key)
}

#[contract]
pub struct TimeLockContract;

#[contractimpl]
impl TimeLockContract {
    pub fn initialize(env: Env, oracle_pk: BytesN<32>) {
        if env.storage().instance().has(&DataKey::Oracle) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Oracle, &oracle_pk);
        env.storage().instance().set(&DataKey::NextGiftId, &1u64);
    }

    pub fn create_gift(
        env: Env,
        sender: Address,
        amount: i128,
        unlock_timestamp: u64,
        recipient_phone_hash: String,
    ) -> u64 {
        sender.require_auth();

        // Check amount limits
        if amount < constants::MIN_GIFT_AMOUNT || amount > constants::MAX_GIFT_AMOUNT {
            panic!("Invalid amount");
        }

        let gift_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextGiftId)
            .unwrap_or(1);

        let gift = Gift {
            sender,
            recipient: None,
            amount,
            unlock_timestamp,
            recipient_phone_hash,
            status: GiftStatus::Created,
        };

        env.storage().instance().set(&DataKey::Gift(gift_id), &gift);
        env.storage()
            .instance()
            .set(&DataKey::NextGiftId, &(gift_id + 1));

        gift_id
    }

    pub fn claim_gift(
        env: Env,
        claimant: Address,
        gift_id: u64,
        verification_proof: BytesN<64>,
    ) -> Result<(), Error> {
        claimant.require_auth();

        let key = DataKey::Gift(gift_id);
        if !env.storage().instance().has(&key) {
            return Err(Error::GiftNotFound);
        }

        let mut gift: Gift = env.storage().instance().get(&key).unwrap();

        // Verify status
        if gift.status != GiftStatus::Created {
            if gift.status == GiftStatus::Claimed {
                return Err(Error::AlreadyClaimed);
            }
            return Err(Error::InvalidStatus);
        }
        
        // Verify Unlock Time
        if env.ledger().timestamp() < gift.unlock_timestamp {
             return Err(Error::NotUnlocked);
        }

        // Verify Oracle Proof
        let oracle_pk: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::Oracle)
            .expect("Contract not initialized");

        // Construct payload: claimant XDR + recipient_phone_hash XDR
        let mut payload = Bytes::new(&env);
        payload.append(&claimant.clone().to_xdr(&env));
        payload.append(&gift.recipient_phone_hash.clone().to_xdr(&env));

        // Verify signature
        env.crypto()
            .ed25519_verify(&oracle_pk, &payload, &verification_proof);

        // Update Gift
        gift.recipient = Some(claimant.clone());
        gift.status = GiftStatus::Claimed;

        env.storage().instance().set(&key, &gift);

        // Emit event
        env.events().publish(
            (symbol_short!("claimed"),),
            (gift_id, claimant, env.ledger().timestamp()),
        );

        Ok(())
    }
}
