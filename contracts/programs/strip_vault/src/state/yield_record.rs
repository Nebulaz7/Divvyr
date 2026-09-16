use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum ExpiryCondition {
    None,
    Timestamp(i64),                        // Unix timestamp after which yield expires
    PayoutCount(u32),                      // Maximum number of payouts to capture
}

#[account]
#[derive(InitSpace)]
pub struct YieldRecord {
    pub vault: Pubkey,                     // 32 bytes: Associated Vault PDA
    pub yield_asset_id: Pubkey,            // 32 bytes: Metaplex Core Asset ID
    pub buyout_price_lamports: u64,        // 8 bytes: Fixed SOL buyout price
    pub share_bps: u16,                    // 2 bytes: Percentage share (e.g. 2000 = 20.00%)
    pub payouts_received_count: u32,       // 4 bytes: Number of payouts distributed
    pub is_active: bool,                   // 1 byte: Active status
    pub bump: u8,                          // 1 byte
    pub expiry_condition: ExpiryCondition, // Enum variant + data

    // Future expansion padding
    pub _reserved: [u8; 32],
}

impl YieldRecord {
    pub const SEED_PREFIX: &'static [u8] = b"yield_record";

    /// Checks if this yield position has passively expired
    pub fn is_expired(&self, current_timestamp: i64) -> bool {
        match self.expiry_condition {
            ExpiryCondition::None => false,
            ExpiryCondition::Timestamp(expiry_ts) => current_timestamp >= expiry_ts,
            ExpiryCondition::PayoutCount(max_payouts) => self.payouts_received_count >= max_payouts,
        }
    }
}
