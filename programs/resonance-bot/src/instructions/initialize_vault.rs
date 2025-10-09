use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    {token_2022::Token2022},
    token_interface::TokenAccount,
    token_interface::Mint,
};
use crate::state::ArbitrageVault;

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        seeds = [ArbitrageVault::SEED, authority.key().as_ref()],
        bump,
        payer = authority,
        space = 8 + ArbitrageVault::LEN  // 8 bytes for discriminator + struct size
    )]
    pub vault: Account<'info, ArbitrageVault>,

    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        associated_token::mint = mint_x,
        associated_token::authority = vault
    )]
    pub vault_ata_x: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        associated_token::mint = mint_y,
        associated_token::authority = vault
    )]
    pub vault_ata_y: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn init_handler(
    ctx: Context<InitializeVault>,
    min_profit_threshold: u64,
    max_single_trade: u64,
) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let bump = ctx.bumps.vault;

    vault.authority = ctx.accounts.authority.key();
    vault.min_profit_threshold = min_profit_threshold;
    vault.max_single_trade = max_single_trade;
    vault.total_profits = 0;
    vault.total_trades = 0;
    vault.failed_trades = 0;
    vault.bump = bump;

    msg!("✅ Vault initialized with authority: {}", vault.authority);
    msg!("   Min profit threshold: {}", min_profit_threshold);
    msg!("   Max single trade: {}", max_single_trade);
    msg!("   Bump: {}", bump);

    Ok(())
}
