use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount, Mint};
use crate::state::{ArbitrageVault, PoolState};
use crate::utils::saros_cpi::{
    SAROS_DLMM_PROGRAM_ID,
    saros_swap_exact_tokens_for_tokens,
    SarosSwapAccounts,
    SwapRequest,
};
use crate::error::ResonanceError;

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
    /// CHECK: Real Saros DLMM pool A
    #[account(
        mut,
        constraint = pool_a.owner == &SAROS_DLMM_PROGRAM_ID @ ResonanceError::InvalidPoolOwner
    )]
    pub pool_a: AccountInfo<'info>,

    /// CHECK: Real Saros DLMM pool B  
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

    // Pool A CPI accounts
    /// CHECK: Pool A user position account
    #[account(mut)]
    pub user_position_a: AccountInfo<'info>,

    /// CHECK: Pool A reserve for input token
    #[account(mut)]
    pub reserve_a_in: AccountInfo<'info>,

    /// CHECK: Pool A reserve for output token
    #[account(mut)]
    pub reserve_a_out: AccountInfo<'info>,

    // Pool B CPI accounts  
    /// CHECK: Pool B user position account
    #[account(mut)]
    pub user_position_b: AccountInfo<'info>,

    /// CHECK: Pool B reserve for input token
    #[account(mut)]
    pub reserve_b_in: AccountInfo<'info>,

    /// CHECK: Pool B reserve for output token
    #[account(mut)]
    pub reserve_b_out: AccountInfo<'info>,

    // Token mints
    pub token_mint_in: Account<'info, Mint>,  // USDC mint
    pub token_mint_out: Account<'info, Mint>, // SAROS mint

    // Additional Saros required accounts
    /// CHECK: Oracle account for price data
    pub oracle: AccountInfo<'info>,

    /// CHECK: Event authority for Saros
    pub event_authority: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: Saros DLMM program
    #[account(constraint = saros_program.key() == SAROS_DLMM_PROGRAM_ID)]
    pub saros_program: AccountInfo<'info>,
}

// Handler function that matches lib.rs signature
pub fn execute_arbitrage_handler(
    ctx: Context<ExecuteArbitrage>,
    _pool_a_key: Pubkey,
    _pool_b_key: Pubkey,
    max_amount_in: Option<u64>,
) -> Result<()> {
    let trade_amount = max_amount_in.unwrap_or(1_000_000); // Default 1 USDC
    execute_arbitrage(ctx, trade_amount)
}

pub fn execute_arbitrage(
    ctx: Context<ExecuteArbitrage>,
    trade_amount: u64,
) -> Result<()> {
    let vault = &mut ctx.accounts.vault;

    msg!("🚀 === REAL SAROS DLMM ARBITRAGE EXECUTION ===");

    // Basic validation
    require!(trade_amount > 0, ResonanceError::InvalidAmount);
    require!(trade_amount <= vault.max_single_trade, ResonanceError::ExceedsMaxTrade);

    let initial_usdc = ctx.accounts.vault_token_y.amount;
    require!(initial_usdc >= trade_amount, ResonanceError::InsufficientFunds);

    // Parse REAL pool states from actual Saros DLMM pool accounts
    let pool_a_state = PoolState::from_account_info(&ctx.accounts.pool_a)?;
    let pool_b_state = PoolState::from_account_info(&ctx.accounts.pool_b)?;

    msg!("📊 Pool A real price: {}", pool_a_state.current_price);
    msg!("📊 Pool B real price: {}", pool_b_state.current_price);
    msg!("📊 Pool A liquidity X: {}, Y: {}", pool_a_state.total_liquidity_x, pool_a_state.total_liquidity_y);
    msg!("📊 Pool B liquidity X: {}, Y: {}", pool_b_state.total_liquidity_x, pool_b_state.total_liquidity_y);

    // Only proceed if there's a real price difference
    require!(
        pool_a_state.current_price != pool_b_state.current_price,
        ResonanceError::NoArbitrageOpportunity
    );

    // Determine swap ordering based on REAL price comparison
    let (cheaper_pool, expensive_pool, cheaper_state, expensive_state, buy_from_a) = 
        if pool_a_state.current_price < pool_b_state.current_price {
            msg!("🎯 ARBITRAGE DIRECTION: Buy from Pool A (cheaper) → Sell to Pool B (expensive)");
            (&ctx.accounts.pool_a, &ctx.accounts.pool_b, &pool_a_state, &pool_b_state, true)
        } else {
            msg!("🎯 ARBITRAGE DIRECTION: Buy from Pool B (cheaper) → Sell to Pool A (expensive)");
            (&ctx.accounts.pool_b, &ctx.accounts.pool_a, &pool_b_state, &pool_a_state, false)
        };

    // Calculate price difference percentage  
    let price_diff_pct = if cheaper_state.current_price > 0 {
        ((expensive_state.current_price - cheaper_state.current_price) as f64 / 
         cheaper_state.current_price as f64) * 100.0
    } else {
        0.0
    };

    msg!("💰 Real price difference: {:.4}%", price_diff_pct);

    // Prepare vault seeds for CPI signing
    let authority_key = ctx.accounts.authority.key();
    let vault_seeds = &[
        ArbitrageVault::SEED,
        authority_key.as_ref(),
        &[vault.bump],
    ];
    let signer_seeds = &[&vault_seeds[..]];

    // Calculate minimum output amounts (basic slippage protection)
    let min_saros_out = trade_amount / 2; // Simple minimum for USDC->SAROS

    // 🚀 EXECUTE FIRST SAROS CPI: USDC → SAROS on cheaper pool
    msg!("⚡ EXECUTING SWAP 1: USDC → SAROS on {} pool", 
        if buy_from_a { "A" } else { "B" });

    execute_saros_swap_cpi(
        if buy_from_a { &ctx.accounts.pool_a } else { &ctx.accounts.pool_b },
        if buy_from_a { &ctx.accounts.user_position_a } else { &ctx.accounts.user_position_b },
        &ctx.accounts.vault_token_y, // USDC source
        &ctx.accounts.vault_token_x, // SAROS destination
        if buy_from_a { &ctx.accounts.reserve_a_in } else { &ctx.accounts.reserve_b_in },
        if buy_from_a { &ctx.accounts.reserve_a_out } else { &ctx.accounts.reserve_b_out },
        &ctx.accounts.token_mint_in,  // USDC mint
        &ctx.accounts.token_mint_out, // SAROS mint
        &ctx.accounts.oracle,
        &ctx.accounts.event_authority,
        &ctx.accounts.saros_program,
        &ctx.accounts.authority,
        &ctx.accounts.token_program,
        trade_amount,
        min_saros_out,
        Some(signer_seeds),
    )?;

    msg!("✅ SWAP 1 COMPLETED: USDC → SAROS");

    // Refresh SAROS balance for second swap
    ctx.accounts.vault_token_x.reload()?;
    let saros_received = ctx.accounts.vault_token_x.amount;
    msg!("💎 SAROS received from first swap: {}", saros_received);

    require!(saros_received > 0, ResonanceError::SwapFailed);

    // Calculate minimum USDC output for second swap
    let min_usdc_out = (saros_received * cheaper_state.current_price) / 1_000_000; // Basic calculation

    // 🚀 EXECUTE SECOND SAROS CPI: SAROS → USDC on expensive pool
    msg!("⚡ EXECUTING SWAP 2: SAROS → USDC on {} pool", 
        if buy_from_a { "B" } else { "A" });

    execute_saros_swap_cpi(
        if buy_from_a { &ctx.accounts.pool_b } else { &ctx.accounts.pool_a },
        if buy_from_a { &ctx.accounts.user_position_b } else { &ctx.accounts.user_position_a },
        &ctx.accounts.vault_token_x, // SAROS source
        &ctx.accounts.vault_token_y, // USDC destination
        if buy_from_a { &ctx.accounts.reserve_b_in } else { &ctx.accounts.reserve_a_in },
        if buy_from_a { &ctx.accounts.reserve_b_out } else { &ctx.accounts.reserve_a_out },
        &ctx.accounts.token_mint_out, // SAROS mint (now input)
        &ctx.accounts.token_mint_in,  // USDC mint (now output)
        &ctx.accounts.oracle,
        &ctx.accounts.event_authority,
        &ctx.accounts.saros_program,
        &ctx.accounts.authority,
        &ctx.accounts.token_program,
        saros_received,
        min_usdc_out,
        Some(signer_seeds),
    )?;

    msg!("✅ SWAP 2 COMPLETED: SAROS → USDC");

    // Refresh final USDC balance
    ctx.accounts.vault_token_y.reload()?;
    let final_usdc = ctx.accounts.vault_token_y.amount;

    // Calculate actual result
    let balance_change = final_usdc as i64 - initial_usdc as i64;
    msg!("💰 Balance change: {} USDC", balance_change);

    // Update vault statistics
    vault.total_trades = vault.total_trades.saturating_add(1);

    if balance_change > 0 {
        vault.total_profits = vault.total_profits.saturating_add(balance_change as u64);
        msg!("📈 Profit realized: +{} USDC", balance_change);
    } else {
        msg!("📉 Loss: {} USDC", balance_change);
    }

    msg!("📊 Vault Statistics:");
    msg!("   Total Trades: {}", vault.total_trades);
    msg!("   Total Profits: {}", vault.total_profits);

    msg!("🎉 === REAL ARBITRAGE EXECUTION COMPLETED ===");

    Ok(())
}

/// Execute actual Saros CPI swap
fn execute_saros_swap_cpi<'info>(
    lb_pair: &AccountInfo<'info>,
    user_position: &AccountInfo<'info>,
    user_token_in: &Account<'info, TokenAccount>,
    user_token_out: &Account<'info, TokenAccount>,
    reserve_in: &AccountInfo<'info>,
    reserve_out: &AccountInfo<'info>,
    token_mint_in: &Account<'info, Mint>,
    token_mint_out: &Account<'info, Mint>,
    oracle: &AccountInfo<'info>,
    event_authority: &AccountInfo<'info>,
    saros_program: &AccountInfo<'info>,
    user: &Signer<'info>,
    token_program: &Program<'info, Token>,
    amount_in: u64,
    minimum_amount_out: u64,
    signer_seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    let swap_request = SwapRequest {
        amount_in,
        minimum_amount_out,
    };

    let swap_accounts = SarosSwapAccounts {
        lb_pair: lb_pair.clone(),
        user_position: user_position.clone(),
        bin_array_bitmap_extension: None,
        user_token_in: user_token_in.clone(),
        user_token_out: user_token_out.clone(),
        reserve_in: reserve_in.clone(),
        reserve_out: reserve_out.clone(),
        token_mint_in: token_mint_in.clone(),
        token_mint_out: token_mint_out.clone(),
        oracle: oracle.clone(),
        host_fee_in: None,
        user: user.clone(),
        token_x_program: token_program.clone(),
        token_y_program: token_program.clone(),
        event_authority: event_authority.clone(),
        program: saros_program.clone(),
    };

    // Execute the actual Saros CPI call
    saros_swap_exact_tokens_for_tokens(
        &swap_accounts,
        swap_request,
        signer_seeds,
    )?;

    Ok(())
}