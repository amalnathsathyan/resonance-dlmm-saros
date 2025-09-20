use anchor_lang::prelude::*;

// Module tree (keep these in sync with files we’ll paste next)
pub mod state;
pub mod utils;
pub mod saros_dlmm;
pub mod instructions;
pub mod error;

use instructions::*;
use error::*;

// Replace with your actual program ID after `anchor keys sync`
declare_id!("85341ciyeLCi7WZ6BhvMBmDyVcih8zNTS4rM7T4KRjqT");

#[program]
pub mod resonance_bot {
    use super::*;

    // Core entrypoint: two-swap CPI arbitrage using Saros DLMM
    pub fn execute_arbitrage(
        ctx: Context<ExecuteArbitrage>,
        pool_a_key: Pubkey,
        pool_b_key: Pubkey,
        max_amount_in: u64,
    ) -> Result<()> {
        instructions::execute_arbitrage::handler(ctx, pool_a_key, pool_b_key, max_amount_in)
    }
}
