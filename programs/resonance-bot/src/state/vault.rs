use anchor_lang::prelude::*;

#[account]
pub struct ArbitrageVault {
    pub authority: Pubkey,           // 32 bytes
    pub min_profit_threshold: u64,   // 8 bytes
    pub max_single_trade: u64,       // 8 bytes
    pub total_profits: u64,          // 8 bytes
    pub total_trades: u64,           // 8 bytes
    pub failed_trades: u64,          // 8 bytes
    pub bump: u8,                    // 1 byte
}

impl ArbitrageVault {
    // Space calculation: 32 + 8 + 8 + 8 + 8 + 8 + 1 = 73 bytes
    // Add padding for alignment: round up to 80
    pub const LEN: usize = 80;
    pub const SEED: &'static [u8] = b"resonance-vault";
}
