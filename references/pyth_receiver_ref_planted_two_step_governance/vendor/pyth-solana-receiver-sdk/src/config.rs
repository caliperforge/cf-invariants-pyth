// CF-PORT: 1:1 vendor of target_chains/solana/pyth_solana_receiver_sdk/src/config.rs;
//   upstream `mod tests` block stripped (minimal vendored surface).
use anchor_lang::prelude::*;

#[account]
#[derive(Debug, PartialEq)]
pub struct Config {
    pub governance_authority: Pubkey, // This authority can update the other fields
    pub target_governance_authority: Option<Pubkey>, // This field is used for a two-step governance authority transfer
    pub wormhole: Pubkey,                            // The address of the wormhole receiver
    pub valid_data_sources: Vec<DataSource>, // The list of valid data sources for oracle price updates
    pub single_update_fee_in_lamports: u64,  // The fee in lamports for a single price update
    pub minimum_signatures: u8, // The minimum number of signatures required to accept a VAA
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Debug)]
pub struct DataSource {
    pub chain: u16,
    pub emitter: Pubkey,
}

impl Config {
    pub const LEN: usize = 370; // This is two times the current size of a Config account with 2 data sources, to leave space for more fields
}
