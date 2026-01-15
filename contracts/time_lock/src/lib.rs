#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Address, String};

mod types;
mod errors;
mod constants;
mod test;

#[contract]
pub struct TimeLockContract;

#[contractimpl]
impl TimeLockContract {
}
