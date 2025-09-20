use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Minimal snapshot of a DLMM pool used by optimal amount and checks.
/// Keep this aligned with how you deserialize the on-chain pool account.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct PoolState {
    /// Q64.64 fixed-point price (quote per base)
    pub current_price: u64,

    /// Base fee rate in basis points (e.g., 100 => 1%)
    pub base_fee_rate: u16,

    /// Bin step parameter for DLMM
    pub bin_step: u16,

    /// Total base token liquidity (X)
    pub total_liquidity_x: u64,

    /// Total quote token liquidity (Y)
    pub total_liquidity_y: u64,

    /// Reserved for future layout compatibility
    pub reserved: [u8; 40],
}

impl PoolState {
    /// Deserialize `PoolState` from account data skipping the Anchor discriminator.
    /// Adjust the offset if your target account is not an Anchor account.
    pub fn from_account_info(account: &AccountInfo) -> Result<&'static Self> {
        let data = account.try_borrow_data()?;
        // Safety: bytemuck cast requires correct layout; ensure your pool account matches this struct.
        Ok(bytemuck::cast_ref(&data[8..]))
    }
}
    