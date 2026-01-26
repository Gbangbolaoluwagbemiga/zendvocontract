#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Bytes, BytesN, Env, String, xdr::ToXdr};
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;

#[test]
fn test_claim_gift() {
    let env = Env::default();
    env.mock_all_auths();

    // 1. Initialize Oracle
    let mut csprng = OsRng;
    let oracle_keypair = SigningKey::generate(&mut csprng);
    let oracle_pub_bytes = oracle_keypair.verifying_key().to_bytes();
    let oracle_pk = BytesN::from_array(&env, &oracle_pub_bytes);

    let contract_id = env.register(TimeLockContract, ());
    let client = TimeLockContractClient::new(&env, &contract_id);

    client.initialize(&oracle_pk);

    // 2. Create Gift
    let sender = Address::generate(&env);
    let recipient_phone_hash = String::from_str(&env, "hash_of_phone_number");
    let amount = 10_000_000;
    let unlock_time = env.ledger().timestamp() + 100;

    let gift_id = client.create_gift(
        &sender,
        &amount,
        &unlock_time,
        &recipient_phone_hash,
    );

    // 3. Prepare Claim
    let claimant = Address::generate(&env);
    
    // Construct payload for signature: claimant XDR + phone hash XDR
    let mut payload = Bytes::new(&env);
    payload.append(&claimant.clone().to_xdr(&env));
    payload.append(&recipient_phone_hash.clone().to_xdr(&env));
    
    // Sign payload
    let len = payload.len() as usize;
    let mut payload_vec = std::vec![0u8; len];
    payload.copy_into_slice(&mut payload_vec);
    
    let signature = oracle_keypair.sign(&payload_vec);
    let signature_bytes = signature.to_bytes();
    let proof = BytesN::from_array(&env, &signature_bytes);

    // 4. Try to claim early (should fail)
    let res = client.try_claim_gift(&claimant, &gift_id, &proof);
    assert!(res.is_err());
    assert_eq!(res.err(), Some(Ok(Error::NotUnlocked)));

    // 5. Advance time
    env.ledger().set_timestamp(unlock_time + 1);

    // 6. Claim successfully
    let res = client.try_claim_gift(&claimant, &gift_id, &proof);
    assert!(res.is_ok());

    // 7. Try to claim again (should fail)
    let res = client.try_claim_gift(&claimant, &gift_id, &proof);
    assert_eq!(res.err(), Some(Ok(Error::AlreadyClaimed)));
}

#[test]
#[should_panic]
fn test_invalid_proof() {
    let env = Env::default();
    env.mock_all_auths();
    
    let mut csprng = OsRng;
    let oracle_keypair = SigningKey::generate(&mut csprng);
    let oracle_pk = BytesN::from_array(&env, &oracle_keypair.verifying_key().to_bytes());

    let contract_id = env.register(TimeLockContract, ());
    let client = TimeLockContractClient::new(&env, &contract_id);

    client.initialize(&oracle_pk);

    let sender = Address::generate(&env);
    let recipient_phone_hash = String::from_str(&env, "hash_of_phone_number");
    let amount = 10_000_000;
    let unlock_time = env.ledger().timestamp(); 

    let gift_id = client.create_gift(
        &sender,
        &amount,
        &unlock_time,
        &recipient_phone_hash,
    );

    let claimant = Address::generate(&env);
    
    // Wrong payload (different phone hash)
    let wrong_hash = String::from_str(&env, "wrong_hash");
    let mut payload = Bytes::new(&env);
    payload.append(&claimant.clone().to_xdr(&env));
    payload.append(&wrong_hash.clone().to_xdr(&env));
    
    let mut payload_vec = std::vec![0u8; payload.len() as usize];
    payload.copy_into_slice(&mut payload_vec);
    
    let signature = oracle_keypair.sign(&payload_vec);
    let proof = BytesN::from_array(&env, &signature.to_bytes());

    // Should panic because of crypto verification failure
    client.claim_gift(&claimant, &gift_id, &proof);
}
