use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};
use anchor_spl::token::{Token, TokenAccount, Mint};

pub const SAROS_DLMM_PROGRAM_ID: Pubkey = pubkey!("1qbkdrr3z4ryLA7pZykqxvxWPoeifcVKo6ZG9CfkvVE");

// Swap instruction discriminator for swap with ExactInput mode
const SWAP_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];

#[derive(Clone, Copy, Debug)]
pub struct SwapParams {
    pub amount_in: u64,
    pub minimum_amount_out: u64,
    pub swap_for_y: bool, // true = X->Y, false = Y->X
}

impl SwapParams {
    pub fn build_instruction_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&SWAP_INSTRUCTION_DISCRIMINATOR);
        data.extend_from_slice(&self.amount_in.to_le_bytes());
        data.extend_from_slice(&self.minimum_amount_out.to_le_bytes());
        data.push(if self.swap_for_y { 1 } else { 0 });
        data
    }
}

pub struct SarosSwapAccounts<'info> {
    pub pair: AccountInfo<'info>,
    pub bin_array_lower: AccountInfo<'info>,
    pub bin_array_upper: AccountInfo<'info>,
    pub user_vault_x: Account<'info, TokenAccount>,
    pub user_vault_y: Account<'info, TokenAccount>,
    pub token_vault_x: AccountInfo<'info>,
    pub token_vault_y: AccountInfo<'info>,
    pub token_mint_x: Account<'info, Mint>,
    pub token_mint_y: Account<'info, Mint>,
    pub token_program_x: Program<'info, Token>,
    pub token_program_y: Program<'info, Token>,
    pub user: Signer<'info>,
    pub event_authority: AccountInfo<'info>,
    pub program: AccountInfo<'info>,
}

impl<'info> SarosSwapAccounts<'info> {
    pub fn get_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.pair.key(), false),
            AccountMeta::new(self.bin_array_lower.key(), false),
            AccountMeta::new(self.bin_array_upper.key(), false),
            AccountMeta::new_readonly(self.token_mint_x.key(), false),
            AccountMeta::new_readonly(self.token_mint_y.key(), false),
            AccountMeta::new(self.token_vault_x.key(), false),
            AccountMeta::new(self.token_vault_y.key(), false),
            AccountMeta::new(self.user_vault_x.key(), false),
            AccountMeta::new(self.user_vault_y.key(), false),
            AccountMeta::new_readonly(self.token_program_x.key(), false),
            AccountMeta::new_readonly(self.token_program_y.key(), false),
            AccountMeta::new_readonly(self.user.key(), true),
            AccountMeta::new_readonly(self.event_authority.key(), false),
            AccountMeta::new_readonly(self.program.key(), false),
        ]
    }

    pub fn get_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![
            self.pair.clone(),
            self.bin_array_lower.clone(),
            self.bin_array_upper.clone(),
            self.token_mint_x.to_account_info(),
            self.token_mint_y.to_account_info(),
            self.token_vault_x.clone(),
            self.token_vault_y.clone(),
            self.user_vault_x.to_account_info(),
            self.user_vault_y.to_account_info(),
            self.token_program_x.to_account_info(),
            self.token_program_y.to_account_info(),
            self.user.to_account_info(),
            self.event_authority.clone(),
            self.program.clone(),
        ]
    }
}

pub fn saros_swap(
    accounts: &SarosSwapAccounts,
    swap_params: SwapParams,
    signer_seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    let ix = Instruction {
        program_id: SAROS_DLMM_PROGRAM_ID,
        accounts: accounts.get_account_metas(),
        data: swap_params.build_instruction_data(),
    };

    let infos = accounts.get_account_infos();

    if let Some(seeds) = signer_seeds {
        invoke_signed(&ix, &infos, seeds)?;
    } else {
        anchor_lang::solana_program::program::invoke(&ix, &infos)?;
    }

    Ok(())
}

// Helper to derive bin array PDAs
pub fn derive_bin_array_pda(pair: &Pubkey, bin_array_index: i64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"bin_array",
            pair.as_ref(),
            &bin_array_index.to_le_bytes(),
        ],
        &SAROS_DLMM_PROGRAM_ID,
    )
}
