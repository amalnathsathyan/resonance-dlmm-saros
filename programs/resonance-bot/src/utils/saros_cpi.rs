// programs/resonance-bot/src/utils/saros_cpi.rs

use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use crate::saros_dlmm;

/// Direction tagging for which pool context to pull accounts from.
#[derive(Debug, Clone, Copy)]
pub enum PoolType {
    PoolA,
    PoolB,
}

/// Execute a Saros DLMM swap CPI using the vault PDA as signer.
/// swap_for_y = true means swapping Y->X (spend quote, buy base), false is X->Y (sell base, get quote).
pub fn execute_saros_swap(
    ctx: &Context<crate::instructions::ExecuteArbitrage>,
    amount_in: u64,
    amount_out_min: u64,
    swap_for_y: bool,
    pool: PoolType,
) -> Result<()> {
    msg!("🔄 Saros DLMM swap | amount_in={} | min_out={} | swap_for_y={}", amount_in, amount_out_min, swap_for_y);

    // Select accounts based on pool side
    let (lb_pair, reserve_x, reserve_y) = match pool {
        PoolType::PoolA => (
            ctx.accounts.pool_a.to_account_info(),
            ctx.accounts.vault_a_x.to_account_info(),
            ctx.accounts.vault_a_y.to_account_info(),
        ),
        PoolType::PoolB => (
            ctx.accounts.pool_b.to_account_info(),
            ctx.accounts.vault_b_x.to_account_info(),
            ctx.accounts.vault_b_y.to_account_info(),
        ),
    };

    // User (program) token accounts (ATAs) controlled by the vault PDA
    let (user_token_x, user_token_y) = (
        ctx.accounts.vault_token_x.to_account_info(),
        ctx.accounts.vault_token_y.to_account_info(),
    );

    // Serialize instruction data as per Saros DLMM swap interface
    let data = saros_dlmm::SwapExactTokensForTokens {
        amount_in,
        amount_out_min,
        swap_for_y,
    }
    .try_to_vec()?;

    // Account metas: validate and reorder as required by Saros DLMM program IDL.
    // NOTE: Adjust ordering to match Saros DLMM exact instruction layout if it differs.
    let accounts = vec![
        AccountMeta::new(lb_pair.key(), false),
        AccountMeta::new(reserve_x.key(), false),
        AccountMeta::new(reserve_y.key(), false),
        AccountMeta::new(user_token_x.key(), false),
        AccountMeta::new(user_token_y.key(), false),
        AccountMeta::new_readonly(ctx.accounts.vault.key(), true), // vault PDA is the authority
        AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
    ];

    let ix = Instruction {
        program_id: saros_dlmm::id(),
        accounts,
        data,
    };

    // Vault PDA signer seeds
    let bump = ctx.accounts.vault.load()?.bump;
    let seeds: &[&[u8]] = &[crate::state::ArbitrageVault::SEED, &[bump]];
    let signer_seeds: &[&[&[u8]]] = &[&seeds];

    // Invoke CPI
    let account_infos = &[
        lb_pair,
        reserve_x,
        reserve_y,
        user_token_x,
        user_token_y,
        ctx.accounts.vault.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
    ];

    anchor_lang::solana_program::program::invoke_signed(&ix, account_infos, signer_seeds)?;

    msg!("✅ Saros DLMM swap CPI completed");
    Ok(())
}
