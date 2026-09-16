use crate::errors::StripVaultError;
use anchor_lang::prelude::*;

/// Scaling factor for fixed-point multiplier representation (9 decimals of precision)
pub const MULTIPLIER_PRECISION: u64 = 1_000_000_000; // 1e9

/// Converts an f64 multiplier from Token-2022 ScaledUiAmountConfig to a fixed-point u64
pub fn f64_to_fixed_multiplier(val: f64) -> Result<u64> {
    if val <= 0.0 || val.is_nan() || val.is_infinite() {
        return err!(StripVaultError::InvalidMultiplier);
    }
    let scaled = (val * (MULTIPLIER_PRECISION as f64)).round();
    if scaled > (u64::MAX as f64) {
        return err!(StripVaultError::MathOverflow);
    }
    Ok(scaled as u64)
}

/// Converts a fixed-point u64 multiplier back to f64
pub fn fixed_multiplier_to_f64(fixed: u64) -> f64 {
    (fixed as f64) / (MULTIPLIER_PRECISION as f64)
}

/// Calculates the exact raw token amount to harvest from a multiplier delta without diluting principal equity value.
///
/// Formula:
///   Raw to Harvest = Current Raw Balance * (New Multiplier - Old Multiplier) / New Multiplier
pub fn calculate_harvest_amount(
    current_raw_balance: u64,
    old_multiplier: u64,
    new_multiplier: u64,
) -> Result<u64> {
    require!(old_multiplier > 0, StripVaultError::InvalidMultiplier);
    require!(new_multiplier > 0, StripVaultError::InvalidMultiplier);
    require!(
        new_multiplier > old_multiplier,
        StripVaultError::NoDividendDelta
    );

    if current_raw_balance == 0 {
        return Ok(0);
    }

    let delta_multiplier = new_multiplier
        .checked_sub(old_multiplier)
        .ok_or(StripVaultError::MathOverflow)?;

    let numerator = (current_raw_balance as u128)
        .checked_mul(delta_multiplier as u128)
        .ok_or(StripVaultError::MathOverflow)?;

    let harvest_amount = numerator
        .checked_div(new_multiplier as u128)
        .ok_or(StripVaultError::MathOverflow)?;

    u64::try_from(harvest_amount).map_err(|_| error!(StripVaultError::MathOverflow))
}

/// Calculates a yield recipient's share of a payout given their allocation in basis points (100 bps = 1.00%)
pub fn calculate_yield_share(total_payout_amount: u64, share_bps: u16) -> Result<u64> {
    require!(
        share_bps <= 10_000,
        StripVaultError::ExceedsMaxYieldAllocation
    );

    if total_payout_amount == 0 || share_bps == 0 {
        return Ok(0);
    }

    let share = (total_payout_amount as u128)
        .checked_mul(share_bps as u128)
        .ok_or(StripVaultError::MathOverflow)?
        .checked_div(10_000)
        .ok_or(StripVaultError::MathOverflow)?;

    u64::try_from(share).map_err(|_| error!(StripVaultError::MathOverflow))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f64_conversions() {
        let fixed = f64_to_fixed_multiplier(1.05).unwrap();
        assert_eq!(fixed, 1_050_000_000);

        let back_to_f64 = fixed_multiplier_to_f64(fixed);
        assert!((back_to_f64 - 1.05).abs() < 1e-9);

        assert!(f64_to_fixed_multiplier(0.0).is_err());
        assert!(f64_to_fixed_multiplier(-1.5).is_err());
        assert!(f64_to_fixed_multiplier(f64::NAN).is_err());
    }

    #[test]
    fn test_worked_proof_100_to_105() {
        // Initial deposit: 100 shares with 6 decimals (100_000_000 raw units)
        let initial_raw = 100_000_000u64;
        let old_mult = f64_to_fixed_multiplier(1.00).unwrap(); // 1_000_000_000
        let new_mult = f64_to_fixed_multiplier(1.05).unwrap(); // 1_050_000_000

        // Calculate harvest tokens
        let harvested = calculate_harvest_amount(initial_raw, old_mult, new_mult).unwrap();

        // 100 * (0.05 / 1.05) = 4.76190476... shares -> 4_761_904 raw units
        assert_eq!(harvested, 4_761_904);

        // Remaining raw tokens in vault
        let remaining_raw = initial_raw - harvested;
        assert_eq!(remaining_raw, 95_238_096);

        // Calculate UI value of remaining tokens at the new multiplier:
        // UI Value = remaining_raw * 1.05 = 99_999_999.8 raw units (~100.00 shares)
        let remaining_ui_value = ((remaining_raw as u128) * (new_mult as u128)) / (MULTIPLIER_PRECISION as u128);
        
        // Difference from original 100.00 shares (100_000_000 units) must be within 1 integer rounding unit
        let rounding_diff = (initial_raw as i64) - (remaining_ui_value as i64);
        assert!(rounding_diff.abs() <= 1, "Rounding diff {} exceeds 1 unit", rounding_diff);
    }

    #[test]
    fn test_zero_raw_balance() {
        let old_mult = f64_to_fixed_multiplier(1.00).unwrap();
        let new_mult = f64_to_fixed_multiplier(1.10).unwrap();
        let harvested = calculate_harvest_amount(0, old_mult, new_mult).unwrap();
        assert_eq!(harvested, 0);
    }

    #[test]
    fn test_no_multiplier_increase_reverts() {
        let old_mult = f64_to_fixed_multiplier(1.05).unwrap();
        let new_mult = f64_to_fixed_multiplier(1.05).unwrap();
        let res = calculate_harvest_amount(100_000_000, old_mult, new_mult);
        assert!(res.is_err());

        let lower_mult = f64_to_fixed_multiplier(1.02).unwrap();
        let res_lower = calculate_harvest_amount(100_000_000, old_mult, lower_mult);
        assert!(res_lower.is_err());
    }

    #[test]
    fn test_large_balance_no_overflow() {
        // 10 billion shares with 6 decimals = 10_000_000_000_000_000 raw units
        let large_balance = 10_000_000_000_000_000u64;
        let old_mult = f64_to_fixed_multiplier(1.00).unwrap();
        let new_mult = f64_to_fixed_multiplier(2.00).unwrap();

        let harvested = calculate_harvest_amount(large_balance, old_mult, new_mult).unwrap();
        // 10B * (1.00 / 2.00) = 5B harvested
        assert_eq!(harvested, 5_000_000_000_000_000u64);
    }

    #[test]
    fn test_yield_shares_basis_points() {
        let total_usdc = 1_000_000_000u64; // 1,000 USDC (6 decimals)
        
        // 25% share (2,500 bps)
        let share_25 = calculate_yield_share(total_usdc, 2500).unwrap();
        assert_eq!(share_25, 250_000_000);

        // 100% share (10,000 bps)
        let share_100 = calculate_yield_share(total_usdc, 10000).unwrap();
        assert_eq!(share_100, 1_000_000_000);

        // 0% share (0 bps)
        let share_0 = calculate_yield_share(total_usdc, 0).unwrap();
        assert_eq!(share_0, 0);

        // >100% share (10,001 bps) should revert
        assert!(calculate_yield_share(total_usdc, 10001).is_err());
    }
}
