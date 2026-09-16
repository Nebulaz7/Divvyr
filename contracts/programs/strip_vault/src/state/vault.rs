use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum PayoutAssetType {
    Usdc,
    Sol,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {
    // Fixed offsets for indexers and memcmp filters
    pub stock_mint: Pubkey,                // 32 bytes: Token-2022 xStock mint
    pub stock_vault_ata: Pubkey,           // 32 bytes: Vault's Token-2022 holding account
    pub master_deed_asset: Pubkey,         // 32 bytes: Metaplex Core Asset ID for Master Deed
    pub payout_holding_ata: Pubkey,        // 32 bytes: ATA for USDC or Vault PDA for native SOL
    pub total_raw_shares: u64,             // 8 bytes: Total raw tokens locked
    pub last_multiplier: u64,              // 8 bytes: Multiplier at last harvest (scaled by 1e9)
    pub pending_harvest_amount: u64,       // 8 bytes: Raw tokens waiting in step 2 swap
    pub total_yield_allocated_bps: u16,    // 2 bytes: Sum of active yield shares (max 10,000)
    pub payout_asset_type: PayoutAssetType,// 1 byte: Usdc (0) or Sol (1)
    pub vault_bump: u8,                    // 1 byte: Canonical bump
    pub vault_seed: u64,                   // 8 bytes: Unique vault discriminator seed

    // Future expansion padding (Solana Foundation solana-dev guideline)
    pub _reserved: [u8; 64],
}

impl Vault {
    pub const SEED_PREFIX: &'static [u8] = b"vault";
    pub const MAX_YIELD_BPS: u16 = 10_000; // 100.00%
}
