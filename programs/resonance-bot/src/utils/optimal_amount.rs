// programs/resonance-bot/src/utils/optimal_amount.rs

use crate::state::PoolState;
use crate::error::ResonanceError;  // Changed from 'errors' to 'error'
use anchor_lang::prelude::*;

/// Calculate the optimal amount of Y (e.g., USDC) to use for arbitrage.
///
/// Derived form:
/// Δy* = min(
/// R_x^A * P_A * (1 - f_A),
/// R_y^B / (1 - f_A) / P_A
/// )
/// with all terms capped by the available vault Y balance to avoid over-spend.
pub fn calculate_optimal_amount_in(
    pool_a: &PoolState, // lower price pool
    pool_b: &PoolState, // higher price pool
    vault_balance_y: u64, // available quote balance (e.g., USDC)
) -> Result<u64> {
    // Ensure price advantage
    require!(
        pool_b.current_price > pool_a.current_price,
        ResonanceError::NoArbitrageOpportunity
    );

    let reserve_x_a = pool_a.total_liquidity_x as u128;
    let reserve_y_b = pool_b.total_liquidity_y as u128;

    // Fees in basis points (e.g., 100 = 1%)
    let fee_bp_a = pool_a.base_fee_rate as u128;
    let fee_mult_a = 10_000u128
        .checked_sub(fee_bp_a)
        .ok_or(ResonanceError::ArithmeticOverflow)?;

    // Price of pool A (lower price)
    let price_a = pool_a.current_price as u128;

    // Constraint 1: R_x^A * P_A * (1 - f_A) / 10000
    let constraint_1 = reserve_x_a
        .checked_mul(price_a)
        .ok_or(ResonanceError::ArithmeticOverflow)?
        .checked_mul(fee_mult_a)
        .ok_or(ResonanceError::ArithmeticOverflow)?
        .checked_div(10_000u128)
        .ok_or(ResonanceError::ArithmeticOverflow)?;

    // Constraint 2: R_y^B * 10000 / ((1 - f_A) * P_A)
    let denominator = fee_mult_a
        .checked_mul(price_a)
        .ok_or(ResonanceError::ArithmeticOverflow)?;

    let constraint_2 = reserve_y_b
        .checked_mul(10_000u128)
        .ok_or(ResonanceError::ArithmeticOverflow)?
        .checked_div(denominator)
        .ok_or(ResonanceError::ArithmeticOverflow)?;

    // Take minimum of constraints
    let optimal_amount = std::cmp::min(constraint_1, constraint_2);

    // Cap by available vault balance
    let capped_amount = std::cmp::min(optimal_amount, vault_balance_y as u128);

    // Ensure non-zero result
    require!(
        capped_amount > 0,
        ResonanceError::InsufficientProfit
    );

    Ok(capped_amount as u64)
}
