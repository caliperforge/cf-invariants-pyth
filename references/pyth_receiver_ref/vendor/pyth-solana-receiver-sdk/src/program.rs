// CF-PORT: 1:1 vendor of target_chains/solana/pyth_solana_receiver_sdk/src/program.rs.
use anchor_lang::prelude::*;

pub struct PythSolanaReceiver;

impl Id for PythSolanaReceiver {
    fn id() -> Pubkey {
        crate::ID
    }
}
