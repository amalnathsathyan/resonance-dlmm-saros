// programs/resonance-bot/src/utils/optimal_amount.rs

use crate::state::PoolState;
use crate::error::ResonanceError;
use anchor_lang::prelude::*;

/// Calculate the optimal amount of Y (e.g., USDC) to use for arbitrage.
///
/// Derived form:
/// Δy* = min(
///   R_x^A * P_A * (1 - f_A),
///   R_y^B / (1 - f_A) / P_A
/// )
/// with all terms capped by the available vault Y balance to avoid over-spend.
pub fn calculate_optimal_amount_in(
    pool_a: &PoolState,      // lower price pool
    pool_b: &PoolState,      // higher price pool
    vault_balance_y: u64,    // available quote balance (e.g., USDC)
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

    // Constraint 1: limited by A's ability to sell X for Y (converted to Y input cap)
    let constraint_1 = reserve_x_a
        .checked_mul(pool_a.current_price as u128)
        .ok_or(ResonanceError::ArithmeticOverflow)?
        .checked_mul(fee_mult_a)
        .ok_or(ResonanceError::ArithmeticOverflow)?
        .checked_div(10_000)
        .ok_or(ResonanceError::ArithmeticOverflow)? as u64;

    // Constraint 2: limited by B's ability to buy X with Y (converted back through price and fees)
    let numerator = reserve_y_b
        .checked_mul(10_000)
        .ok_or(ResonanceError::ArithmeticOverflow)?;
    let denom = fee_mult_a
        .checked_mul(pool_a.current_price as u128)
        .ok_or(ResonanceError::ArithmeticOverflow)?;
    let constraint_2 = numerator
        .checked_div(denom)
        .ok_or(ResonanceError::ArithmeticOverflow)? as u64;

    let raw_optimal = constraint_1.min(constraint_2);
    let optimal = raw_optimal.min(vault_balance_y);

    msg!("OptimalAmountIn:");
    msg!("  constraint_1 (A cap): {}", constraint_1);
    msg!("  constraint_2 (B cap): {}", constraint_2);
    msg!("  vault_balance_y: {}", vault_balance_y);
    msg!("  raw_optimal: {}", raw_optimal);
    msg!("  optimal: {}", optimal);

    require!(optimal > 0, ResonanceError::NoArbitrageOpportunity);
    Ok(optimal)
}
