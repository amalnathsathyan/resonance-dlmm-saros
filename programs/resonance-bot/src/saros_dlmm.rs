// programs/resonance-bot/src/saros_dlmm.rs

use anchor_lang::prelude::*;

// Saros DLMM Program ID (verify against official docs before mainnet-fork tests)
declare_id!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

/// Minimal swap payload for manual CPI; update if Saros DLMM changes its interface.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SwapExactTokensForTokens {
    /// Input amount
    pub amount_in: u64,
    /// Minimum acceptable output (slippage protection)
    pub amount_out_min: u64,
    /// true: swap Y->X (spend quote, buy base), false: X->Y (sell base, get quote)
    pub swap_for_y: bool,
}
