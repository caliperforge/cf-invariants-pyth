// CF-PORT: 1:1 vendor of target_chains/solana/pyth_solana_receiver_sdk/src/lib.rs.
// CF-PORT: upstream `pub mod cpi;` removed — receiver does not import the SDK's CPI helpers
//   and the cpi module is not part of the receiver's compile graph (minimal vendored surface).
// We can't do much about the size of `anchor_lang::error::Error`.
#![allow(clippy::result_large_err)]

use {
    anchor_lang::{declare_id, prelude::*},
    // CF-PORT: anchor-lang 1.0.1 re-exports `borsh` from its prelude; qualify our direct
    // borsh-0.10 import with the crate-root path so the BorshSerialize/BorshDeserialize
    // derives resolve unambiguously (upstream picked the workspace borsh, we pick ours).
    ::borsh::{BorshDeserialize, BorshSerialize},
    pythnet_sdk::wire::v1::MerklePriceUpdate,
};

pub mod config;
pub mod error;
pub mod pda;
pub mod price_update;
pub mod program;

cfg_if::cfg_if! {
    if #[cfg(feature = "pro-compatible")] {
        declare_id!("rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp");
        pub const PYTH_PUSH_ORACLE_ID: Pubkey = pubkey!("pyt2F414BA6dPttK6RddPZUdHfapoBN24GL5wbrPCou");
    } else {
        declare_id!("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ");
        pub const PYTH_PUSH_ORACLE_ID: Pubkey = pubkey!("pythWSnswVUd12oZpeFP8e9CVaEqJg25g1Vtc2biRsT");
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct PostUpdateParams {
    pub merkle_price_update: MerklePriceUpdate,
    pub treasury_id: u8,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct PostUpdateAtomicParams {
    pub vaa: Vec<u8>,
    pub merkle_price_update: MerklePriceUpdate,
    pub treasury_id: u8,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct PostTwapUpdateParams {
    pub start_merkle_price_update: MerklePriceUpdate,
    pub end_merkle_price_update: MerklePriceUpdate,
    pub treasury_id: u8,
}
