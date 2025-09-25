use anchor_lang::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct PoolState {
    pub current_price: u64,
    pub base_fee_rate: u16,
    pub bin_step: u16,
    pub active_bin_id: i32,
    pub total_liquidity_x: u64,
    pub total_liquidity_y: u64,
    pub reserved: [u8; 32],
}

impl PoolState {
    pub fn from_account_info(account: &AccountInfo) -> Result<Self> {
        let data = account.try_borrow_data()?;

        if data.len() < 80 {
            return Err(error!(crate::error::ResonanceError::InvalidProgram));
        }

        Ok(Self {
            current_price: u64::from_le_bytes([
                data[8], data[9], data[10], data[11],
                data[12], data[13], data[14], data[15]
            ]),
            base_fee_rate: 30,
            bin_step: 25,
            active_bin_id: 0,
            total_liquidity_x: u64::from_le_bytes([
                data[32], data[33], data[34], data[35],
                data[36], data[37], data[38], data[39]
            ]),
            total_liquidity_y: u64::from_le_bytes([
                data[64], data[65], data[66], data[67],
                data[68], data[69], data[70], data[71]
            ]),
            reserved: [0; 32],
        })
    }
}
