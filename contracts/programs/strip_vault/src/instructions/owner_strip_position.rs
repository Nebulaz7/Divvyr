use crate::errors::StripVaultError;
use crate::events::YieldStrippedEvent;
use crate::state::{ExpiryCondition, Vault, YieldRecord};
use crate::utils::metaplex_core::mint_yield_nft;
use anchor_lang::prelude::*;
use mpl_core::accounts::BaseAssetV1;

#[derive(Accounts)]
#[instruction(
    share_bps: u16,
    expiry_condition: ExpiryCondition,
    buyout_price_lamports: u64,
    name: String,
    uri: String
)]
pub struct OwnerStripPosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: Validated against vault.master_deed_asset and deserialized as BaseAssetV1 in handler
    pub master_deed_asset: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [Vault::SEED_PREFIX, vault.stock_mint.as_ref(), &vault.vault_seed.to_le_bytes()],
        bump = vault.vault_bump
    )]
    pub vault: Account<'info, Vault>,

    /// Metaplex Core Yield Asset keypair to be created and minted
    #[account(mut)]
    pub yield_asset: Signer<'info>,

    /// Recipient of the Yield NFT (can be creator, friend, family member)
    /// CHECK: Valid recipient address
    pub recipient: AccountInfo<'info>,

    #[account(
        init,
        payer = owner,
        space = 8 + YieldRecord::INIT_SPACE,
        seeds = [YieldRecord::SEED_PREFIX, vault.key().as_ref(), yield_asset.key().as_ref()],
        bump
    )]
    pub yield_record: Account<'info, YieldRecord>,

    /// CHECK: Metaplex Core program verified by address
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<OwnerStripPosition>,
    share_bps: u16,
    expiry_condition: ExpiryCondition,
    buyout_price_lamports: u64,
    name: String,
    uri: String,
) -> Result<()> {
    // 1. Verify caller owns the Master Deed NFT
    let vault = &ctx.accounts.vault;
    require_keys_eq!(
        ctx.accounts.master_deed_asset.key(),
        vault.master_deed_asset,
        StripVaultError::InvalidAssetAccount
    );
    require_keys_eq!(
        *ctx.accounts.master_deed_asset.owner,
        mpl_core::ID,
        StripVaultError::InvalidAssetAccount
    );

    let asset_data = ctx.accounts.master_deed_asset.try_borrow_data()?;
    let master_deed = BaseAssetV1::from_bytes(&asset_data)
        .map_err(|_| StripVaultError::InvalidAssetAccount)?;

    require_keys_eq!(
        master_deed.owner,
        ctx.accounts.owner.key(),
        StripVaultError::UnauthorizedMasterDeedHolder
    );

    // 2. Validate basis points allocation (must be > 0 and cumulative <= 100%)
    require!(share_bps > 0, StripVaultError::ExceedsMaxYieldAllocation);
    let new_total_allocated = vault
        .total_yield_allocated_bps
        .checked_add(share_bps)
        .ok_or(StripVaultError::MathOverflow)?;

    require!(
        new_total_allocated <= Vault::MAX_YIELD_BPS,
        StripVaultError::ExceedsMaxYieldAllocation
    );

    // 3. Mint Yield NFT via Metaplex Core CPI with PermanentBurnDelegate attached to the Vault PDA
    mint_yield_nft(
        &ctx.accounts.mpl_core_program,
        &ctx.accounts.yield_asset,
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.recipient,
        &ctx.accounts.vault.to_account_info(),
        name,
        uri,
    )?;

    // 4. Initialize YieldRecord PDA state
    let yield_record = &mut ctx.accounts.yield_record;
    yield_record.vault = vault.key();
    yield_record.yield_asset_id = ctx.accounts.yield_asset.key();
    yield_record.buyout_price_lamports = buyout_price_lamports;
    yield_record.share_bps = share_bps;
    yield_record.payouts_received_count = 0;
    yield_record.is_active = true;
    yield_record.bump = ctx.bumps.yield_record;
    yield_record.expiry_condition = expiry_condition;
    yield_record._reserved = [0u8; 32];

    // 5. Update Vault's total allocated basis points
    let vault_mut = &mut ctx.accounts.vault;
    vault_mut.total_yield_allocated_bps = new_total_allocated;

    emit!(YieldStrippedEvent {
        vault: vault_mut.key(),
        yield_asset_id: yield_record.yield_asset_id,
        recipient: ctx.accounts.recipient.key(),
        share_bps,
        total_allocated_bps: new_total_allocated,
    });

    Ok(())
}
