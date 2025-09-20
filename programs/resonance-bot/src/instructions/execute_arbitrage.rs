use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::state::{ArbitrageVault, PoolState};
use crate::utils::{optimal_amount, saros_cpi};
use crate::error::ResonanceError;
use crate::saros_dlmm;

#[derive(Accounts)]
pub struct ExecuteArbitrage<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // Vault PDA as CPI signer
    #[account(
        mut,
        seeds = [ArbitrageVault::SEED],
        bump = vault.load()?.bump,
    )]
    pub vault: AccountLoader<'info, ArbitrageVault>,

    // Pool A (candidate lower price)
    /// CHECK: Validated by owner check
    #[account(
        constraint = pool_a.owner == &saros_dlmm::id() @ ResonanceError::InvalidPoolOwner
    )]
    pub pool_a: AccountInfo<'info>,
    /// CHECK: Pool A reserves (layout validated via Saros docs/tests)
    pub vault_a_x: AccountInfo<'info>,
    /// CHECK: Pool A reserves (layout validated via Saros docs/tests)
    pub vault_a_y: AccountInfo<'info>,

    // Pool B (candidate higher price)
    /// CHECK: Validated by owner check
    #[account(
        constraint = pool_b.owner == &saros_dlmm::id() @ ResonanceError::InvalidPoolOwner
    )]
    pub pool_b: AccountInfo<'info>,
    /// CHECK: Pool B reserves
    pub vault_b_x: AccountInfo<'info>,
    /// CHECK: Pool B reserves
    pub vault_b_y: AccountInfo<'info>,

    // Vault ATAs for base (X) and quote (Y, e.g., USDC)
    #[account(mut)]
    pub vault_token_x: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_token_y: Account<'info, TokenAccount>,

    // Programs
    /// CHECK: Saros DLMM program id guard
    #[account(
        constraint = saros_dlmm_program.key == &saros_dlmm::id() @ ResonanceError::InvalidProgram
    )]
    pub saros_dlmm_program: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<ExecuteArbitrage>,
    pool_a_key: Pubkey,
    pool_b_key: Pubkey,
    max_amount_in: u64,
) -> Result<()> {
    let mut vault = ctx.accounts.vault.load_mut()?;

    // Early guards
    require!(max_amount_in > 0, ResonanceError::InvalidAmount);
    require!(max_amount_in <= vault.max_single_trade, ResonanceError::ExceedsMaxTrade);

    // Load pool snapshots (zero-copy from account data)
    let pool_a_state = PoolState::from_account_info(&ctx.accounts.pool_a)?;
    let pool_b_state = PoolState::from_account_info(&ctx.accounts.pool_b)?;

    // Determine direction by on-chain prices (buy low, sell high)
    let (low_state, high_state, buy_pool, sell_pool) = if pool_a_state.current_price < pool_b_state.current_price {
        (pool_a_state, pool_b_state, saros_cpi::PoolType::PoolA, saros_cpi::PoolType::PoolB)
    } else {
        (pool_b_state, pool_a_state, saros_cpi::PoolType::PoolB, saros_cpi::PoolType::PoolA)
    };

    // Vault quote balance (Y)
    let initial_y = ctx.accounts.vault_token_y.amount;

    // Optimal amount capped by vault balance
    let mut optimal = optimal_amount::calculate_optimal_amount_in(
        low_state,
        high_state,
        initial_y,
    )?;

    // Also cap by the requested max input cap (defense-in-depth)
    if optimal > max_amount_in {
        optimal = max_amount_in;
    }

    msg!("Arb route: buy on {:?}, sell on {:?}, amount_in_y={}", buy_pool, sell_pool, optimal);

    // 1) Buy X using Y on lower-price pool
    saros_cpi::execute_saros_swap(
        &ctx,
        optimal,        // spend Y
        0,              // min out = 0 for MVP; tighten later
        true,           // swap_for_y = true => Y -> X
        buy_pool,
    )?;

    let received_x = ctx.accounts.vault_token_x.amount;
    msg!("Received X after buy: {}", received_x);

    // 2) Sell X for Y on higher-price pool
    saros_cpi::execute_saros_swap(
        &ctx,
        received_x,     // spend X
        0,              // min out = 0 for MVP; tighten later
        false,          // swap_for_y = false => X -> Y
        sell_pool,
    )?;

    // Profit verification
    let final_y = ctx.accounts.vault_token_y.amount;
    let profit = final_y
        .checked_sub(initial_y)
        .ok_or(ResonanceError::ArithmeticOverflow)?;

    msg!("Final Y: {}, Profit: {}", final_y, profit);
    require!(profit >= vault.min_profit_threshold, ResonanceError::ProfitNotRealized);

    // Update stats
    vault.total_profits = vault.total_profits
        .checked_add(profit)
        .ok_or(ResonanceError::ArithmeticOverflow)?;
    vault.total_trades = vault.total_trades
        .checked_add(1)
        .ok_or(ResonanceError::ArithmeticOverflow)?;

    Ok(())
}
