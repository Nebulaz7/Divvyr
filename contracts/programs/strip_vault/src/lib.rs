use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

pub use errors::*;
pub use events::*;
pub use instructions::*;
pub use state::*;
pub use utils::*;

declare_id!("Divvyr1111111111111111111111111111111111111");

#[program]
pub mod strip_vault {
    use super::*;

    /// Initializes a new vault with deposited Token-2022 stock shares and mints the Master Deed NFT to the creator
    pub fn creator_initialize_vault(
        ctx: Context<CreatorInitializeVault>,
        amount: u64,
        payout_asset_type: PayoutAssetType,
        vault_seed: u64,
        name: String,
        uri: String,
    ) -> Result<()> {
        instructions::creator_initialize_vault::handler(
            ctx,
            amount,
            payout_asset_type,
            vault_seed,
            name,
            uri,
        )
    }

    /// Carves out a percentage of future dividend rights and mints a bearer Yield NFT to the recipient with BurnDelegate attached
    pub fn owner_strip_position(
        ctx: Context<OwnerStripPosition>,
        share_bps: u16,
        expiry_condition: ExpiryCondition,
        buyout_price_lamports: u64,
        name: String,
        uri: String,
    ) -> Result<()> {
        instructions::owner_strip_position::handler(
            ctx,
            share_bps,
            expiry_condition,
            buyout_price_lamports,
            name,
            uri,
        )
    }

    /// Step 1: Harvests raw stock tokens from a multiplier bump without diluting principal equity value
    pub fn keeper_harvest_dividend(ctx: Context<KeeperHarvestDividend>) -> Result<()> {
        instructions::keeper_harvest_dividend::handler(ctx)
    }

    /// Step 2: Deterministic simulated swap of pending harvest tokens against the reserve into USDC or SOL
    pub fn keeper_execute_swap(
        ctx: Context<KeeperExecuteSwap>,
        custom_rate: Option<u64>,
    ) -> Result<()> {
        instructions::keeper_execute_swap::handler(ctx, custom_rate)
    }

    /// Step 3: Passive expiry check, holder verification, and proportional payout distribution to active Yield NFTs & Master Deed
    pub fn keeper_distribute_payout<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, KeeperDistributePayout<'info>>,
        total_payout_amount: u64,
    ) -> Result<()> {
        instructions::keeper_distribute_payout::handler(ctx, total_payout_amount)
    }

    pub fn ping(_ctx: Context<Ping>) -> Result<()> {
        msg!("Divvyr strip_vault program live!");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Ping<'info> {
    pub signer: Signer<'info>,
}
