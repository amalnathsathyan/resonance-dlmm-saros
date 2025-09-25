use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, Mint, Token, TokenAccount, Transfer},
};
use crate::state::ArbitrageVault;

#[derive(Accounts)]
pub struct DepositFunds<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [ArbitrageVault::SEED, authority.key().as_ref()],
        bump = vault.bump,
        has_one = authority
    )]
    pub vault: Account<'info, ArbitrageVault>,

    pub mint_x: Account<'info, Mint>,

    #[account(mut)]
    pub authority_ata_x: Account<'info, TokenAccount>,

    #[account(mut)]
    pub vault_ata_x: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn deposit_funds_handler(
    ctx: Context<DepositFunds>,
    amount_x: u64,
    _amount_y: u64,
) -> Result<()> {
    let vault = &ctx.accounts.vault;

    if amount_x > 0 {
        // Avoid temporary value lifetime issues
        let auth_key = ctx.accounts.authority.key();
        let bump = vault.bump;
        let seeds: [&[u8]; 3] = [
            ArbitrageVault::SEED,
            auth_key.as_ref(),
            &[bump],
        ];
        let signer_seeds: &[&[&[u8]]] = &[&seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.authority_ata_x.to_account_info(),
                    to: ctx.accounts.vault_ata_x.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
                signer_seeds,
            ),
            amount_x,
        )?;
    }

    Ok(())
}
