use anchor_lang::prelude::*;

#[event]
pub struct VaultInitializedEvent {
    pub vault: Pubkey,
    pub stock_mint: Pubkey,
    pub master_deed_asset: Pubkey,
    pub initial_shares: u64,
    pub initial_multiplier: u64,
}

#[event]
pub struct YieldStrippedEvent {
    pub vault: Pubkey,
    pub yield_asset_id: Pubkey,
    pub recipient: Pubkey,
    pub share_bps: u16,
    pub total_allocated_bps: u16,
}

#[event]
pub struct DividendHarvestedEvent {
    pub vault: Pubkey,
    pub old_multiplier: u64,
    pub new_multiplier: u64,
    pub harvested_tokens: u64,
}

#[event]
pub struct PayoutDistributedEvent {
    pub vault: Pubkey,
    pub total_payout: u64,
    pub master_deed_share: u64,
    pub yield_holders_count: u32,
}

#[event]
pub struct YieldBoughtOutEvent {
    pub vault: Pubkey,
    pub yield_asset_id: Pubkey,
    pub recipient_paid: Pubkey,
    pub buyout_price_lamports: u64,
    pub share_bps_reclaimed: u16,
}

#[event]
pub struct SwapExecutedEvent {
    pub vault: Pubkey,
    pub stock_amount_in: u64,
    pub payout_amount_out: u64,
}
