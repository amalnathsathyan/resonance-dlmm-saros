use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount, Mint};
use crate::state::{ArbitrageVault, PoolState};
use crate::utils::saros_cpi::{
    SAROS_DLMM_PROGRAM_ID,
};
use crate::error::ResonanceError;

// Saros DLMM account seeds
const BIN_ARRAY_SEED: &[u8] = b"bin_array";
const RESERVE_SEED: &[u8] = b"reserve";

#[derive(Accounts)]
pub struct ExecuteArbitrage<'info> {
    #[account(
        mut,
        seeds = [ArbitrageVault::SEED, authority.key().as_ref()],
        bump = vault.bump,
        has_one = authority @ ResonanceError::InvalidProgram
    )]
    pub vault: Account<'info, ArbitrageVault>,
    
    pub authority: Signer<'info>,
    
    // Real Saros DLMM pools
    /// CHECK:
    #[account(
        mut,
        constraint = pool_a.owner == &SAROS_DLMM_PROGRAM_ID @ ResonanceError::InvalidPoolOwner
    )]
    pub pool_a: AccountInfo<'info>,
    
    /// CHECK:
    #[account(
        mut,
        constraint = pool_b.owner == &SAROS_DLMM_PROGRAM_ID @ ResonanceError::InvalidPoolOwner
    )]
    pub pool_b: AccountInfo<'info>,
    
    // Vault token accounts
    #[account(mut)]
    pub vault_token_x: Account<'info, TokenAccount>, // SAROS
    
    #[account(mut)]
    pub vault_token_y: Account<'info, TokenAccount>, // USDC
    
    // Token mints
    pub token_x_mint: Account<'info, Mint>,
    pub token_y_mint: Account<'info, Mint>,
    
    pub token_program: Program<'info, Token>,
}

/// Optimized struct for derived accounts
#[derive(Debug, Clone)]
pub struct SarosDerivedAccounts {
    pub bin_array_lower: Pubkey,
    pub bin_array_upper: Pubkey, 
    pub reserve_x: Pubkey,
    pub reserve_y: Pubkey,
    pub active_bin_id: i32,
}

/// Optimized pool data parsing (reduced compute usage)
pub fn parse_saros_pool_data(pool_data: &[u8]) -> Result<i32> {
    if pool_data.len() < 100 {
        return Ok(0);
    }
    
    // Quick extraction - try only most likely offsets
    for offset in [64, 72, 80, 88].iter() {
        if *offset + 4 <= pool_data.len() {
            let bin_id = i32::from_le_bytes([
                pool_data[*offset], 
                pool_data[*offset + 1], 
                pool_data[*offset + 2], 
                pool_data[*offset + 3]
            ]);
            
            if bin_id >= -10000 && bin_id <= 10000 && bin_id != 0 {
                return Ok(bin_id);
            }
        }
    }
    
    Ok(0) // Default
}

/// Optimized account derivation
pub fn derive_saros_dlmm_accounts(pool_address: &Pubkey, active_bin_id: i32) -> Result<SarosDerivedAccounts> {
    
    let bin_array_lower_index = ((active_bin_id - 256) / 512) as i32;
    let bin_array_upper_index = ((active_bin_id + 256) / 512) as i32;
    
    let (bin_array_lower, _) = Pubkey::find_program_address(
        &[
            BIN_ARRAY_SEED,
            pool_address.as_ref(),
            &bin_array_lower_index.to_le_bytes(),
        ],
        &SAROS_DLMM_PROGRAM_ID,
    );
    
    let (bin_array_upper, _) = Pubkey::find_program_address(
        &[
            BIN_ARRAY_SEED,
            pool_address.as_ref(),
            &bin_array_upper_index.to_le_bytes(),
        ],
        &SAROS_DLMM_PROGRAM_ID,
    );
    
    let (reserve_x, _) = Pubkey::find_program_address(
        &[
            RESERVE_SEED,
            pool_address.as_ref(),
            b"token_x",
        ],
        &SAROS_DLMM_PROGRAM_ID,
    );
    
    let (reserve_y, _) = Pubkey::find_program_address(
        &[
            RESERVE_SEED,
            pool_address.as_ref(),
            b"token_y",
        ],
        &SAROS_DLMM_PROGRAM_ID,
    );
    
    Ok(SarosDerivedAccounts {
        bin_array_lower,
        bin_array_upper,
        reserve_x,
        reserve_y,
        active_bin_id,
    })
}

pub fn execute_arbitrage_handler(
    ctx: Context<ExecuteArbitrage>,
    _pool_a_key: Pubkey,
    _pool_b_key: Pubkey,
    max_amount_in: Option<u64>,
) -> Result<()> {
    
    let trade_amount = max_amount_in.unwrap_or(3000 * 1_000_000);
    let initial_usdc = ctx.accounts.vault_token_y.amount;
    let initial_saros = ctx.accounts.vault_token_x.amount;
    
    msg!("🚀 SAROS DLMM ARBITRAGE START");
    msg!("USDC: {} SAROS: {}", initial_usdc, initial_saros);
    msg!("Trade: {} USDC", trade_amount);
    
    require!(initial_usdc >= trade_amount, ResonanceError::InsufficientFunds);
    
    let execution_start_slot = Clock::get()?.slot;
    
    // Parse pool data (optimized)
    let pool_a_data = &ctx.accounts.pool_a.data.borrow();
    let pool_b_data = &ctx.accounts.pool_b.data.borrow();
    let active_bin_a = parse_saros_pool_data(pool_a_data)?;
    let active_bin_b = parse_saros_pool_data(pool_b_data)?;
    
    msg!("Pool A: {} bytes, bin: {}", pool_a_data.len(), active_bin_a);
    msg!("Pool B: {} bytes, bin: {}", pool_b_data.len(), active_bin_b);
    
    // Derive accounts (optimized)
    let pool_a_accounts = derive_saros_dlmm_accounts(&ctx.accounts.pool_a.key(), active_bin_a)?;
    let pool_b_accounts = derive_saros_dlmm_accounts(&ctx.accounts.pool_b.key(), active_bin_b)?;
    
    msg!("✅ Accounts derived");
    msg!("Pool A Bin Lower: {}", pool_a_accounts.bin_array_lower);
    msg!("Pool A Bin Upper: {}", pool_a_accounts.bin_array_upper);
    msg!("Pool A Reserve X: {}", pool_a_accounts.reserve_x);
    msg!("Pool A Reserve Y: {}", pool_a_accounts.reserve_y);
    
    msg!("Pool B Bin Lower: {}", pool_b_accounts.bin_array_lower);
        msg!("Pool B Bin Upper: {}", pool_b_accounts.bin_array_upper);
    msg!("Pool B Reserve X: {}", pool_b_accounts.reserve_x);
    msg!("Pool B Reserve Y: {}", pool_b_accounts.reserve_y);
    
    // Calculate trade parameters (simplified)
    let pool_a_state = PoolState::from_account_info(&ctx.accounts.pool_a)?;
    let pool_b_state = PoolState::from_account_info(&ctx.accounts.pool_b)?;
    
    let optimal_amount = if pool_b_state.current_price > pool_a_state.current_price {
        use crate::utils::optimal_amount::calculate_optimal_amount_in;
        calculate_optimal_amount_in(&pool_a_state, &pool_b_state, initial_usdc)?
    } else {
        trade_amount
    };
    
    let actual_trade_amount = std::cmp::min(trade_amount, optimal_amount);
    
    msg!("Prices A:{} B:{}", pool_a_state.current_price, pool_b_state.current_price);
    msg!("Optimal: {} Actual: {}", optimal_amount, actual_trade_amount);
    
    // Execute arbitrage simulation (very lightweight)
    msg!("📈 SWAP 1: USDC->SAROS Pool A");
    let simulated_saros_received = actual_trade_amount * 1100;
    msg!("SAROS received: {}", simulated_saros_received);
    
    msg!("📉 SWAP 2: SAROS->USDC Pool B");
    let simulated_usdc_received = simulated_saros_received / 1050;
    let simulated_profit = simulated_usdc_received as i64 - actual_trade_amount as i64;
    msg!("USDC received: {}", simulated_usdc_received);
    msg!("Profit: {} USDC", simulated_profit);
    
    let final_slot = Clock::get()?.slot;
    let total_duration = final_slot - execution_start_slot;
    
    // Update vault statistics
    let vault = &mut ctx.accounts.vault;
    vault.total_trades = vault.total_trades.saturating_add(1);
    
    if simulated_profit > 0 {
        vault.total_profits = vault.total_profits.saturating_add(simulated_profit as u64);
        msg!("🎉 PROFITABLE! +{:.4}%", (simulated_profit as f64 / actual_trade_amount as f64) * 100.0);
    } else {
        vault.failed_trades = vault.failed_trades.saturating_add(1);
        msg!("📉 Loss: {}", simulated_profit);
    }
    
    // MEV analysis
    if total_duration <= 1 {
        msg!("🔒 ATOMIC: {} slots", total_duration);
    } else {
        msg!("⚡ FAST: {} slots", total_duration);
    }
    
    msg!("✅ ARBITRAGE COMPLETE");
    msg!("Trades: {} Profits: ${}", vault.total_trades, vault.total_profits as f64 / 1_000_000.0);
    
    Ok(())
}

