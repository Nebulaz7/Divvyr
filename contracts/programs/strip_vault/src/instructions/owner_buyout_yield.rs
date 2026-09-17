use crate::errors::StripVaultError;
use crate::events::YieldBoughtOutEvent;
use crate::state::{Vault, YieldRecord};
use crate::utils::metaplex_core::burn_yield_nft_as_delegate;
use anchor_lang::prelude::*;
use mpl_core::accounts::BaseAssetV1;

#[derive(Accounts)]
pub struct OwnerBuyoutYield<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: Validated against vault.master_deed_asset and deserialized as BaseAssetV1
    pub master_deed_asset: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [Vault::SEED_PREFIX, vault.stock_mint.as_ref(), &vault.vault_seed.to_le_bytes()],
        bump = vault.vault_bump,
    )]
    pub vault: Account<'info, Vault>,

    /// Metaplex Core Yield Asset account to be burned
    /// CHECK: Validated against yield_record.yield_asset_id and mpl_core::ID
    #[account(mut)]
    pub yield_asset: AccountInfo<'info>,

    /// Wallet of the current on-chain owner of the Yield NFT, to receive buyout SOL
    /// CHECK: Validated against on-chain owner of yield_asset
    #[account(mut)]
    pub yield_nft_holder_wallet: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [YieldRecord::SEED_PREFIX, vault.key().as_ref(), yield_asset.key().as_ref()],
        bump = yield_record.bump,
    )]
    pub yield_record: Account<'info, YieldRecord>,

    /// CHECK: Metaplex Core program verified by address
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<OwnerBuyoutYield>) -> Result<()> {
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

    let master_deed_data = ctx.accounts.master_deed_asset.try_borrow_data()?;
    let master_deed = BaseAssetV1::from_bytes(&master_deed_data)
        .map_err(|_| StripVaultError::InvalidAssetAccount)?;

    require_keys_eq!(
        master_deed.owner,
        ctx.accounts.owner.key(),
        StripVaultError::UnauthorizedMasterDeedHolder
    );
    drop(master_deed_data);

    // 2. Validate YieldRecord is active and belongs to this vault & asset
    let yield_record = &ctx.accounts.yield_record;
    require_keys_eq!(
        yield_record.vault,
        vault.key(),
        StripVaultError::InvalidYieldRecord
    );
    require_keys_eq!(
        yield_record.yield_asset_id,
        ctx.accounts.yield_asset.key(),
        StripVaultError::InvalidYieldRecord
    );
    require!(
        yield_record.is_active,
        StripVaultError::YieldPositionInactive
    );

    // 3. Verify Yield Asset on Metaplex Core and validate current holder wallet
    require_keys_eq!(
        *ctx.accounts.yield_asset.owner,
        mpl_core::ID,
        StripVaultError::InvalidAssetAccount
    );
    let yield_asset_data = ctx.accounts.yield_asset.try_borrow_data()?;
    let yield_asset_struct = BaseAssetV1::from_bytes(&yield_asset_data)
        .map_err(|_| StripVaultError::InvalidAssetAccount)?;
    let current_holder = yield_asset_struct.owner;
    drop(yield_asset_data);

    require_keys_eq!(
        ctx.accounts.yield_nft_holder_wallet.key(),
        current_holder,
        StripVaultError::InvalidRecipientWallet
    );

    // 4. Transfer buyout SOL from caller to Yield NFT holder
    let buyout_price = yield_record.buyout_price_lamports;
    if buyout_price > 0 {
        let transfer_cpi = anchor_lang::system_program::Transfer {
            from: ctx.accounts.owner.to_account_info(),
            to: ctx.accounts.yield_nft_holder_wallet.to_account_info(),
        };
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                transfer_cpi,
            ),
            buyout_price,
        )?;
    }

    // 5. Vault PDA executes delegated burn of the Yield NFT
    let vault_seed_bytes = vault.vault_seed.to_le_bytes();
    let vault_seeds: &[&[u8]] = &[
        Vault::SEED_PREFIX,
        vault.stock_mint.as_ref(),
        &vault_seed_bytes,
        &[vault.vault_bump],
    ];
    let vault_signer = &[vault_seeds];

    burn_yield_nft_as_delegate(
        &ctx.accounts.mpl_core_program,
        &ctx.accounts.yield_asset,
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        vault_signer,
    )?;

    // 6. Update YieldRecord and Vault state
    let share_bps_reclaimed = yield_record.share_bps;
    let yield_asset_id = yield_record.yield_asset_id;

    let yield_record_mut = &mut ctx.accounts.yield_record;
    yield_record_mut.is_active = false;

    let vault_mut = &mut ctx.accounts.vault;
    vault_mut.total_yield_allocated_bps = vault_mut
        .total_yield_allocated_bps
        .saturating_sub(share_bps_reclaimed);

    emit!(YieldBoughtOutEvent {
        vault: vault_mut.key(),
        yield_asset_id,
        recipient_paid: ctx.accounts.yield_nft_holder_wallet.key(),
        buyout_price_lamports: buyout_price,
        share_bps_reclaimed,
    });

    Ok(())
}
