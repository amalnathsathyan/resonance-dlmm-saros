use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};
use anchor_spl::token::{Token, TokenAccount, Mint};

pub const SAROS_DLMM_PROGRAM_ID: Pubkey = pubkey!("1qbkdrr3z4ryLA7pZykqxvxWPoeifcVKo6ZG9CfkvVE");

const SWAP_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [0xc1, 0x8f, 0xd8, 0x7e, 0x3e, 0x6f, 0x5a, 0xd8];

#[derive(Clone, Copy, Debug)]
pub struct SwapRequest {
    pub amount_in: u64,
    pub minimum_amount_out: u64,
}

impl SwapRequest {
    pub fn build_instruction_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&SWAP_INSTRUCTION_DISCRIMINATOR);
        data.extend_from_slice(&self.amount_in.to_le_bytes());
        data.extend_from_slice(&self.minimum_amount_out.to_le_bytes());
        data
    }
}

pub struct SarosSwapAccounts<'info> {
    pub lb_pair: AccountInfo<'info>,
    pub user_position: AccountInfo<'info>,
    pub bin_array_bitmap_extension: Option<AccountInfo<'info>>,
    pub user_token_in: Account<'info, TokenAccount>,
    pub user_token_out: Account<'info, TokenAccount>,
    pub reserve_in: AccountInfo<'info>,
    pub reserve_out: AccountInfo<'info>,
    pub token_mint_in: Account<'info, Mint>,
    pub token_mint_out: Account<'info, Mint>,
    pub oracle: AccountInfo<'info>,
    pub host_fee_in: Option<Account<'info, TokenAccount>>,
    pub user: Signer<'info>,
    pub token_x_program: Program<'info, Token>,
    pub token_y_program: Program<'info, Token>,
    pub event_authority: AccountInfo<'info>,
    pub program: AccountInfo<'info>,
}

impl<'info> SarosSwapAccounts<'info> {
    pub fn get_account_metas(&self) -> Vec<AccountMeta> {
        let mut metas = vec![
            AccountMeta::new(self.lb_pair.key(), false),
            AccountMeta::new(self.user_position.key(), false),
            AccountMeta::new(self.user_token_in.key(), false),
            AccountMeta::new(self.user_token_out.key(), false),
            AccountMeta::new(self.reserve_in.key(), false),
            AccountMeta::new(self.reserve_out.key(), false),
            AccountMeta::new_readonly(self.token_mint_in.key(), false),
            AccountMeta::new_readonly(self.token_mint_out.key(), false),
            AccountMeta::new_readonly(self.oracle.key(), false),
            AccountMeta::new_readonly(self.user.key(), true),
            AccountMeta::new_readonly(self.token_x_program.key(), false),
            AccountMeta::new_readonly(self.token_y_program.key(), false),
            AccountMeta::new_readonly(self.event_authority.key(), false),
            AccountMeta::new_readonly(self.program.key(), false),
        ];
        if let Some(b) = &self.bin_array_bitmap_extension {
            metas.push(AccountMeta::new(b.key(), false));
        }
        if let Some(hf) = &self.host_fee_in {
            metas.push(AccountMeta::new(hf.key(), false));
        }
        metas
    }

    pub fn get_account_infos(&self) -> Vec<AccountInfo<'info>> {
        let mut infos = vec![
            self.lb_pair.clone(),
            self.user_position.clone(),
            self.user_token_in.to_account_info(),
            self.user_token_out.to_account_info(),
            self.reserve_in.to_account_info(),
            self.reserve_out.to_account_info(),
            self.token_mint_in.to_account_info(),
            self.token_mint_out.to_account_info(),
            self.oracle.clone(),
            self.user.to_account_info(),
            self.token_x_program.to_account_info(),
            self.token_y_program.to_account_info(),
            self.event_authority.clone(),
            self.program.clone(),
        ];
        if let Some(b) = &self.bin_array_bitmap_extension {
            infos.push(b.clone());
        }
        if let Some(hf) = &self.host_fee_in {
            infos.push(hf.to_account_info());
        }
        infos
    }
}

pub fn saros_swap_exact_tokens_for_tokens(
    accounts: &SarosSwapAccounts,
    swap_request: SwapRequest,
    signer_seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    let ix = Instruction {
        program_id: SAROS_DLMM_PROGRAM_ID,
        accounts: accounts.get_account_metas(),
        data: swap_request.build_instruction_data(),
    };
    let infos = accounts.get_account_infos();
    if let Some(seeds) = signer_seeds {
        invoke_signed(&ix, &infos, seeds)?;
    } else {
        anchor_lang::solana_program::program::invoke(&ix, &infos)?;
    }
    Ok(())
}
