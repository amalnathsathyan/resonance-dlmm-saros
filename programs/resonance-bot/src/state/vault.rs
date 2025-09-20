use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

/// PDA holding configuration, stats, and ATA addresses owned by the vault.
/// The vault PDA is the swap authority and signer for CPI.
#[account(zero_copy)]
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct ArbitrageVault {
    /// Vault controller (DAO or user)
    pub authority: Pubkey,

    /// Minimum profit in quote units (e.g., USDC) required per trade
    pub min_profit_threshold: u64,

    /// Max input per trade in quote units (cap before optimal calculation)
    pub max_single_trade: u64,

    /// Cumulative profit in quote token units
    pub total_profits: u64,

    /// Number of successful trades
    pub total_trades: u64,

    /// Number of failed attempts (e.g., profit threshold not met)
    pub failed_trades: u64,

    /// PDA bump
    pub bump: u8,

    /// ATA for base token (X) owned by this vault
    pub ata_x: Pubkey,

    /// ATA for quote token (Y; e.g., USDC) owned by this vault
    pub ata_y: Pubkey,

    /// Reserved for future upgrades (alignment padding)
    pub reserved: [u8; 47],
}

impl ArbitrageVault {
    /// Account size: discriminator + fields
    pub const LEN: usize = 8  // discriminator
        + 32 // authority
        + 8  // min_profit_threshold
        + 8  // max_single_trade
        + 8  // total_profits
        + 8  // total_trades
        + 8  // failed_trades
        + 1  // bump
        + 32 // ata_x
        + 32 // ata_y
        + 47; // reserved

    /// PDA seed
    pub const SEED: &'static [u8] = b"resonance-vault";
}
