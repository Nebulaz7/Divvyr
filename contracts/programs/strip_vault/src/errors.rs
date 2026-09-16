use anchor_lang::prelude::*;

#[error_code]
pub enum StripVaultError {
    #[msg("Calculation overflow in dividend harvesting math")]
    MathOverflow,

    #[msg("Current multiplier is not greater than the last recorded multiplier")]
    NoDividendDelta,

    #[msg("Total allocated yield cannot exceed 100% (10,000 bps)")]
    ExceedsMaxYieldAllocation,

    #[msg("Caller does not own the Master Deed NFT")]
    UnauthorizedMasterDeedHolder,

    #[msg("Yield position is already inactive or expired")]
    YieldPositionInactive,

    #[msg("Cannot redeem principal while active yield positions remain")]
    ActiveYieldPositionsRemain,

    #[msg("No pending dividend tokens to swap")]
    NoPendingHarvest,

    #[msg("Invalid Metaplex Core asset account")]
    InvalidAssetAccount,

    #[msg("Recipient wallet does not match the on-chain owner of the Yield NFT")]
    InvalidRecipientWallet,

    #[msg("Swap reserve has insufficient liquidity")]
    InsufficientReserveLiquidity,

    #[msg("Invalid Token-2022 ScaledUiAmountConfig extension")]
    InvalidScaledUiAmountConfig,

    #[msg("Multiplier is zero or invalid")]
    InvalidMultiplier,
}
