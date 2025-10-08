use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount, Mint};
use crate::state::{ArbitrageVault, PoolState};
use crate::utils::saros_cpi::{
    SAROS_DLMM_PROGRAM_ID,
    saros_swap,
    SarosSwapAccounts,
    SwapParams,
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

    // Pool A accounts
    /// CHECK: Saros DLMM pool A
    #[account(mut)]
    pub pool_a: AccountInfo<'info>,
    /// CHECK: Pool A bin array lower
    #[account(mut)]
    pub bin_array_lower_a: AccountInfo<'info>,
    /// CHECK: Pool A bin array upper
    #[account(mut)]
    pub bin_array_upper_a: AccountInfo<'info>,
    /// CHECK: Pool A token vault X
    #[account(mut)]
    pub token_vault_x_a: AccountInfo<'info>,
    /// CHECK: Pool A token vault Y
    #[account(mut)]
    pub token_vault_y_a: AccountInfo<'info>,

    // Pool B accounts
    /// CHECK: Saros DLMM pool B
    #[account(mut)]
    pub pool_b: AccountInfo<'info>,
    /// CHECK: Pool B bin array lower
    #[account(mut)]
    pub bin_array_lower_b: AccountInfo<'info>,
    /// CHECK: Pool B bin array upper
    #[account(mut)]
    pub bin_array_upper_b: AccountInfo<'info>,
    /// CHECK: Pool B token vault X
    #[account(mut)]
    pub token_vault_x_b: AccountInfo<'info>,
    /// CHECK: Pool B token vault Y
    #[account(mut)]
    pub token_vault_y_b: AccountInfo<'info>,

    // Vault token accounts
    #[account(mut)]
    pub vault_token_x: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_token_y: Account<'info, TokenAccount>,

    // Token mints
    pub token_mint_x: Account<'info, Mint>,
    pub token_mint_y: Account<'info, Mint>,

    /// CHECK: Event authority
    pub event_authority: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,

    /// CHECK: Saros program
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
    let trade_amount = max_amount_in.unwrap_or(100_000_000); // Default 100 USDC
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
    let buy_from_a = pool_a_state.current_price < pool_b_state.current_price;
    
    let (cheaper_state, expensive_state) = if buy_from_a {
        msg!("🎯 ARBITRAGE DIRECTION: Buy from Pool A (cheaper) → Sell to Pool B (expensive)");
        (&pool_a_state, &pool_b_state)
    } else {
        msg!("🎯 ARBITRAGE DIRECTION: Buy from Pool B (cheaper) → Sell to Pool A (expensive)");
        (&pool_b_state, &pool_a_state)
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
        if buy_from_a { &ctx.accounts.bin_array_lower_a } else { &ctx.accounts.bin_array_lower_b },
        if buy_from_a { &ctx.accounts.bin_array_upper_a } else { &ctx.accounts.bin_array_upper_b },
        &ctx.accounts.vault_token_x,
        &ctx.accounts.vault_token_y,
        if buy_from_a { &ctx.accounts.token_vault_x_a } else { &ctx.accounts.token_vault_x_b },
        if buy_from_a { &ctx.accounts.token_vault_y_a } else { &ctx.accounts.token_vault_y_b },
        &ctx.accounts.token_mint_x,
        &ctx.accounts.token_mint_y,
        &ctx.accounts.event_authority,
        &ctx.accounts.saros_program,
        &ctx.accounts.authority,
        &ctx.accounts.token_program,
        trade_amount,
        min_saros_out,
        true, // swap_for_y = true (Y->X direction, USDC->SAROS)
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
        if buy_from_a { &ctx.accounts.bin_array_lower_b } else { &ctx.accounts.bin_array_lower_a },
        if buy_from_a { &ctx.accounts.bin_array_upper_b } else { &ctx.accounts.bin_array_upper_a },
        &ctx.accounts.vault_token_x,
        &ctx.accounts.vault_token_y,
        if buy_from_a { &ctx.accounts.token_vault_x_b } else { &ctx.accounts.token_vault_x_a },
        if buy_from_a { &ctx.accounts.token_vault_y_b } else { &ctx.accounts.token_vault_y_a },
        &ctx.accounts.token_mint_x,
        &ctx.accounts.token_mint_y,
        &ctx.accounts.event_authority,
        &ctx.accounts.saros_program,
        &ctx.accounts.authority,
        &ctx.accounts.token_program,
        saros_received,
        min_usdc_out,
        false, // swap_for_y = false (X->Y direction, SAROS->USDC)
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

#[allow(clippy::too_many_arguments)]
fn execute_saros_swap_cpi<'info>(
    pair: &AccountInfo<'info>,
    bin_array_lower: &AccountInfo<'info>,
    bin_array_upper: &AccountInfo<'info>,
    user_vault_x: &Account<'info, TokenAccount>,
    user_vault_y: &Account<'info, TokenAccount>,
    token_vault_x: &AccountInfo<'info>,
    token_vault_y: &AccountInfo<'info>,
    token_mint_x: &Account<'info, Mint>,
    token_mint_y: &Account<'info, Mint>,
    event_authority: &AccountInfo<'info>,
    saros_program: &AccountInfo<'info>,
    user: &Signer<'info>,
    token_program: &Program<'info, Token>,
    amount_in: u64,
    minimum_amount_out: u64,
    swap_for_y: bool,
    signer_seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    let swap_params = SwapParams {
        amount_in,
        minimum_amount_out,
        swap_for_y,
    };

    let swap_accounts = SarosSwapAccounts {
        pair: pair.clone(),
        bin_array_lower: bin_array_lower.clone(),
        bin_array_upper: bin_array_upper.clone(),
        user_vault_x: user_vault_x.clone(),
        user_vault_y: user_vault_y.clone(),
        token_vault_x: token_vault_x.clone(),
        token_vault_y: token_vault_y.clone(),
        token_mint_x: token_mint_x.clone(),
        token_mint_y: token_mint_y.clone(),
        token_program_x: token_program.clone(),
        token_program_y: token_program.clone(),
        user: user.clone(),
        event_authority: event_authority.clone(),
        program: saros_program.clone(),
    };

    saros_swap(&swap_accounts, swap_params, signer_seeds)?;
    Ok(())
}
