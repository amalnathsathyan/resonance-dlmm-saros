#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;

pub mod instructions;
pub mod state;
pub mod utils;
pub mod error;

pub use instructions::*;

declare_id!("AhTopKWSdP3wE4aBfWtp2tjJHRvAy4JVkfycPsPDW2kx");

#[program]

pub mod resonance_bot {
    use super::*;

    pub fn initialize_vault(
        ctx: Context<InitializeVault>,
        min_profit_threshold: u64,
        max_single_trade: u64,
    ) -> Result<()> {
        init_handler(ctx, min_profit_threshold, max_single_trade)
    }

    pub fn deposit_funds(
        ctx: Context<DepositFunds>,
        amount_x: u64,
    ) -> Result<()> {
        deposit_funds_handler(ctx, amount_x)
    }

    pub fn execute_arbitrage(
        ctx: Context<ExecuteArbitrage>,
        pool_a_key: Pubkey,
        pool_b_key: Pubkey,
        max_amount_in: Option<u64>,
    ) -> Result<()> {
        execute_arbitrage_handler(ctx, pool_a_key, pool_b_key, max_amount_in)
    }
}
