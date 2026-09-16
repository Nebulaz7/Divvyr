use crate::errors::StripVaultError;
use crate::utils::math::f64_to_fixed_multiplier;
use anchor_lang::prelude::*;

/// ExtensionType::ScaledUiAmount discriminator in Token-2022
pub const SCALED_UI_AMOUNT_EXTENSION_TYPE: u16 = 20;

/// Expected byte length of ScaledUiAmountConfig in TLV
pub const SCALED_UI_CONFIG_LEN: usize = 56;

/// Offset in Token-2022 Mint data where TLV extensions start
pub const MINT_EXTENSIONS_OFFSET: usize = 83;

/// Offset in standard SPL Mint data for decimals (1 byte)
pub const MINT_DECIMALS_OFFSET: usize = 44;

/// On-chain layout for Token-2022 ScaledUiAmountConfig
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScaledUiAmountConfig {
    /// Authority that can update the multiplier (32 bytes)
    pub authority: [u8; 32],
    /// Current multiplier as IEEE-754 f64 (8 bytes, little-endian)
    pub multiplier: [u8; 8],
    /// Unix timestamp at which new_multiplier becomes effective (8 bytes, little-endian i64)
    pub new_multiplier_effective_timestamp: [u8; 8],
    /// Next multiplier, once effective timestamp is reached (8 bytes, little-endian f64)
    pub new_multiplier: [u8; 8],
}

impl ScaledUiAmountConfig {
    /// Deserializes a ScaledUiAmountConfig from a 56-byte slice
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SCALED_UI_CONFIG_LEN {
            return err!(StripVaultError::InvalidScaledUiAmountConfig);
        }

        let mut authority = [0u8; 32];
        authority.copy_from_slice(&bytes[0..32]);

        let mut multiplier = [0u8; 8];
        multiplier.copy_from_slice(&bytes[32..40]);

        let mut new_multiplier_effective_timestamp = [0u8; 8];
        new_multiplier_effective_timestamp.copy_from_slice(&bytes[40..48]);

        let mut new_multiplier = [0u8; 8];
        new_multiplier.copy_from_slice(&bytes[48..56]);

        Ok(Self {
            authority,
            multiplier,
            new_multiplier_effective_timestamp,
            new_multiplier,
        })
    }

    /// Returns the currently active multiplier as an f64 based on the Solana clock timestamp
    pub fn get_effective_multiplier_f64(&self, current_unix_timestamp: i64) -> f64 {
        let effective_ts = i64::from_le_bytes(self.new_multiplier_effective_timestamp);
        if effective_ts > 0 && current_unix_timestamp >= effective_ts {
            f64::from_le_bytes(self.new_multiplier)
        } else {
            f64::from_le_bytes(self.multiplier)
        }
    }

    /// Returns the currently active multiplier converted to fixed-point u64 (scaled by 1e9)
    pub fn get_effective_multiplier_fixed(&self, current_unix_timestamp: i64) -> Result<u64> {
        let f64_mult = self.get_effective_multiplier_f64(current_unix_timestamp);
        f64_to_fixed_multiplier(f64_mult)
    }
}

/// Unpacks the decimals and ScaledUiAmountConfig from a Token-2022 Mint account data slice
pub fn unpack_scaled_ui_mint(mint_data: &[u8]) -> Result<(u8, ScaledUiAmountConfig)> {
    if mint_data.len() < MINT_EXTENSIONS_OFFSET {
        return err!(StripVaultError::InvalidScaledUiAmountConfig);
    }

    let decimals = mint_data[MINT_DECIMALS_OFFSET];

    // Scan TLV extensions starting at offset 83
    let mut offset = MINT_EXTENSIONS_OFFSET;
    while offset + 4 <= mint_data.len() {
        let ext_type = u16::from_le_bytes([mint_data[offset], mint_data[offset + 1]]);
        let ext_len = u16::from_le_bytes([mint_data[offset + 2], mint_data[offset + 3]]) as usize;
        offset += 4;

        if offset + ext_len > mint_data.len() {
            break;
        }

        // Check for ScaledUiAmount extension (type = 20 or matching length 56)
        if ext_type == SCALED_UI_AMOUNT_EXTENSION_TYPE && ext_len == SCALED_UI_CONFIG_LEN {
            let config_bytes = &mint_data[offset..offset + SCALED_UI_CONFIG_LEN];
            let config = ScaledUiAmountConfig::from_bytes(config_bytes)?;
            return Ok((decimals, config));
        }

        offset += ext_len;
    }

    err!(StripVaultError::InvalidScaledUiAmountConfig)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unpack_mock_scaled_ui_mint() {
        // Build a mock 83-byte mint buffer + TLV extension
        let mut buffer = vec![0u8; MINT_EXTENSIONS_OFFSET];
        buffer[MINT_DECIMALS_OFFSET] = 6; // 6 decimals

        // Append TLV Header: type 20, len 56
        buffer.extend_from_slice(&20u16.to_le_bytes());
        buffer.extend_from_slice(&56u16.to_le_bytes());

        // Config payload: multiplier = 1.05, scheduled 1.10 at timestamp 1000
        let mut config_bytes = [0u8; 56];
        config_bytes[32..40].copy_from_slice(&1.05f64.to_le_bytes());
        config_bytes[40..48].copy_from_slice(&1000i64.to_le_bytes());
        config_bytes[48..56].copy_from_slice(&1.10f64.to_le_bytes());

        buffer.extend_from_slice(&config_bytes);

        let (decimals, config) = unpack_scaled_ui_mint(&buffer).unwrap();
        assert_eq!(decimals, 6);

        // Before timestamp 1000: multiplier is 1.05
        assert_eq!(config.get_effective_multiplier_f64(500), 1.05);
        assert_eq!(
            config.get_effective_multiplier_fixed(500).unwrap(),
            1_050_000_000
        );

        // At or after timestamp 1000: multiplier is 1.10
        assert_eq!(config.get_effective_multiplier_f64(1000), 1.10);
        assert_eq!(
            config.get_effective_multiplier_fixed(1000).unwrap(),
            1_100_000_000
        );
    }
}
