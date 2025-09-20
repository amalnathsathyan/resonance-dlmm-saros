use anchor_lang::prelude::*;

#[error_code]
pub enum ResonanceError {
    #[msg("Invalid pool owner - expected Saros DLMM program")]
    InvalidPoolOwner,

    #[msg("Invalid program ID provided")]
    InvalidProgram,

    #[msg("Invalid amount - must be greater than 0")]
    InvalidAmount,

    #[msg("Amount exceeds maximum single trade limit")]
    ExceedsMaxTrade,

    #[msg("Insufficient profit opportunity")]
    InsufficientProfit,

    #[msg("Final profit not realized")]
    ProfitNotRealized,

    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("No arbitrage opportunity available")]
    NoArbitrageOpportunity,
}
