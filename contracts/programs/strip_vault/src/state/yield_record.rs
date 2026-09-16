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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expiry_condition_none() {
        let record = YieldRecord {
            vault: Pubkey::default(),
            yield_asset_id: Pubkey::default(),
            buyout_price_lamports: 1_000_000_000,
            share_bps: 2500,
            payouts_received_count: 50,
            is_active: true,
            bump: 255,
            expiry_condition: ExpiryCondition::None,
            _reserved: [0u8; 32],
        };
        assert!(!record.is_expired(i64::MAX));
    }

    #[test]
    fn test_expiry_condition_timestamp() {
        let record = YieldRecord {
            vault: Pubkey::default(),
            yield_asset_id: Pubkey::default(),
            buyout_price_lamports: 1_000_000_000,
            share_bps: 2500,
            payouts_received_count: 0,
            is_active: true,
            bump: 255,
            expiry_condition: ExpiryCondition::Timestamp(1_700_000_000),
            _reserved: [0u8; 32],
        };

        // Before expiry timestamp: active
        assert!(!record.is_expired(1_699_999_999));
        // Exactly at expiry timestamp: expired
        assert!(record.is_expired(1_700_000_000));
        // After expiry timestamp: expired
        assert!(record.is_expired(1_700_000_001));
    }

    #[test]
    fn test_expiry_condition_payout_count() {
        let mut record = YieldRecord {
            vault: Pubkey::default(),
            yield_asset_id: Pubkey::default(),
            buyout_price_lamports: 1_000_000_000,
            share_bps: 2500,
            payouts_received_count: 0,
            is_active: true,
            bump: 255,
            expiry_condition: ExpiryCondition::PayoutCount(4), // 4 quarterly dividends
            _reserved: [0u8; 32],
        };

        assert!(!record.is_expired(1000));
        record.payouts_received_count = 3;
        assert!(!record.is_expired(1000));
        record.payouts_received_count = 4;
        assert!(record.is_expired(1000));
        record.payouts_received_count = 5;
        assert!(record.is_expired(1000));
    }
}
