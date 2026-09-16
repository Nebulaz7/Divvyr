use crate::errors::StripVaultError;
use crate::events::PayoutDistributedEvent;
use crate::state::{PayoutAssetType, Vault, YieldRecord};
use crate::utils::math::calculate_yield_share;
use anchor_lang::prelude::*;
use mpl_core::accounts::BaseAssetV1;

#[derive(Accounts)]
#[instruction(total_payout_amount: u64)]
pub struct KeeperDistributePayout<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [Vault::SEED_PREFIX, vault.stock_mint.as_ref(), &vault.vault_seed.to_le_bytes()],
        bump = vault.vault_bump,
    )]
    pub vault: Account<'info, Vault>,

    /// CHECK: Validated against vault.master_deed_asset and deserialized as BaseAssetV1
    #[account(address = vault.master_deed_asset)]
    pub master_deed_asset: AccountInfo<'info>,

    /// CHECK: Payout account for the Master Deed owner (wallet for SOL, ATA for USDC)
    #[account(mut)]
    pub master_deed_payout_account: AccountInfo<'info>,

    /// CHECK: Payout holding source (vault.payout_holding_ata for USDC, vault PDA for SOL)
    #[account(mut)]
    pub vault_payout_source: AccountInfo<'info>,

    /// CHECK: SPL Token program (used if USDC payout)
    pub token_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    // Remaining accounts: triples of [YieldRecord PDA, Metaplex Core Asset, Recipient Payout Account]
}

pub fn handler<'a, 'b, 'c, 'info>(
    ctx: Context<'a, 'b, 'c, 'info, KeeperDistributePayout<'info>>,
    total_payout_amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;

    // 1. Verify Master Deed NFT on Metaplex Core and determine owner
    require_keys_eq!(
        *ctx.accounts.master_deed_asset.owner,
        mpl_core::ID,
        StripVaultError::InvalidAssetAccount
    );
    let master_deed_bytes = ctx.accounts.master_deed_asset.try_borrow_data()?;
    let master_deed = BaseAssetV1::from_bytes(&master_deed_bytes)
        .map_err(|_| StripVaultError::InvalidAssetAccount)?;
    let master_deed_owner = master_deed.owner;
    drop(master_deed_bytes);

    // 2. Validate Master Deed payout recipient matches on-chain owner
    match ctx.accounts.vault.payout_asset_type {
        PayoutAssetType::Sol => {
            require_keys_eq!(
                ctx.accounts.master_deed_payout_account.key(),
                master_deed_owner,
                StripVaultError::InvalidRecipientWallet
            );
        }
        PayoutAssetType::Usdc => {
            let token_acc_data = ctx.accounts.master_deed_payout_account.try_borrow_data()?;
            let master_deed_token_acc =
                anchor_spl::token::TokenAccount::try_deserialize(&mut &token_acc_data[..])?;
            require_keys_eq!(
                master_deed_token_acc.owner,
                master_deed_owner,
                StripVaultError::InvalidRecipientWallet
            );
        }
    }

    // 3. Determine payout amount to distribute
    let payout_to_distribute = if total_payout_amount > 0 {
        total_payout_amount
    } else {
        match ctx.accounts.vault.payout_asset_type {
            PayoutAssetType::Usdc => {
                let token_acc_data = ctx.accounts.vault_payout_source.try_borrow_data()?;
                let holding_acc =
                    anchor_spl::token::TokenAccount::try_deserialize(&mut &token_acc_data[..])?;
                holding_acc.amount
            }
            PayoutAssetType::Sol => {
                let rent_exempt = Rent::get()?.minimum_balance(8 + Vault::INIT_SPACE);
                let current_lamports = ctx.accounts.vault.to_account_info().lamports();
                current_lamports.saturating_sub(rent_exempt)
            }
        }
    };

    require!(
        payout_to_distribute > 0,
        StripVaultError::NoPendingHarvest
    );

    // Validate available balance covers payout_to_distribute
    match ctx.accounts.vault.payout_asset_type {
        PayoutAssetType::Usdc => {
            let token_acc_data = ctx.accounts.vault_payout_source.try_borrow_data()?;
            let holding_acc =
                anchor_spl::token::TokenAccount::try_deserialize(&mut &token_acc_data[..])?;
            require!(
                holding_acc.amount >= payout_to_distribute,
                StripVaultError::InsufficientPayoutBalance
            );
        }
        PayoutAssetType::Sol => {
            let rent_exempt = Rent::get()?.minimum_balance(8 + Vault::INIT_SPACE);
            let current_lamports = ctx.accounts.vault.to_account_info().lamports();
            let available_sol = current_lamports.saturating_sub(rent_exempt);
            require!(
                available_sol >= payout_to_distribute,
                StripVaultError::InsufficientPayoutBalance
            );
        }
    }

    // 4. Setup Vault PDA signer seeds
    let vault_key = ctx.accounts.vault.key();
    let vault_bump = ctx.accounts.vault.vault_bump;
    let stock_mint = ctx.accounts.vault.stock_mint;
    let vault_seed_bytes = ctx.accounts.vault.vault_seed.to_le_bytes();
    let vault_seeds: &[&[u8]] = &[
        Vault::SEED_PREFIX,
        stock_mint.as_ref(),
        &vault_seed_bytes,
        &[vault_bump],
    ];
    let vault_signer = &[vault_seeds];

    // 5. Process remaining accounts (triples: [YieldRecord, Metaplex Asset, Recipient Account])
    require!(
        ctx.remaining_accounts.len() % 3 == 0,
        StripVaultError::InvalidRemainingAccounts
    );

    let mut total_distributed_to_yield_holders: u64 = 0;
    let mut active_yield_holders_count: u32 = 0;

    for chunk in ctx.remaining_accounts.chunks(3) {
        let yield_record_info = &chunk[0];
        let yield_asset_info = &chunk[1];
        let recipient_payout_info = &chunk[2];

        // Verify YieldRecord PDA derivation
        let (expected_yield_record_pda, _) = Pubkey::find_program_address(
            &[
                YieldRecord::SEED_PREFIX,
                vault_key.as_ref(),
                yield_asset_info.key.as_ref(),
            ],
            ctx.program_id,
        );
        require_keys_eq!(
            yield_record_info.key(),
            expected_yield_record_pda,
            StripVaultError::InvalidYieldRecord
        );

        // Deserialize YieldRecord
        let mut yield_record_data = yield_record_info.try_borrow_mut_data()?;
        let mut yield_record = YieldRecord::try_deserialize(&mut &yield_record_data[..])?;

        require_keys_eq!(
            yield_record.vault,
            vault_key,
            StripVaultError::InvalidYieldRecord
        );
        require_keys_eq!(
            yield_record.yield_asset_id,
            yield_asset_info.key(),
            StripVaultError::InvalidYieldRecord
        );

        // If already inactive, continue
        if !yield_record.is_active {
            continue;
        }

        // Check passive expiry
        if yield_record.is_expired(clock.unix_timestamp) {
            yield_record.is_active = false;
            let vault_mut = &mut ctx.accounts.vault;
            vault_mut.total_yield_allocated_bps = vault_mut
                .total_yield_allocated_bps
                .saturating_sub(yield_record.share_bps);

            yield_record.try_serialize(&mut &mut yield_record_data[..])?;
            continue;
        }

        // Strict recipient validation: must match current owner of Yield NFT
        require_keys_eq!(
            *yield_asset_info.owner,
            mpl_core::ID,
            StripVaultError::InvalidAssetAccount
        );
        let asset_bytes = yield_asset_info.try_borrow_data()?;
        let asset = BaseAssetV1::from_bytes(&asset_bytes)
            .map_err(|_| StripVaultError::InvalidAssetAccount)?;
        let nft_owner = asset.owner;
        drop(asset_bytes);

        match ctx.accounts.vault.payout_asset_type {
            PayoutAssetType::Sol => {
                require_keys_eq!(
                    recipient_payout_info.key(),
                    nft_owner,
                    StripVaultError::InvalidRecipientWallet
                );
            }
            PayoutAssetType::Usdc => {
                let token_acc_data = recipient_payout_info.try_borrow_data()?;
                let recipient_token_acc =
                    anchor_spl::token::TokenAccount::try_deserialize(&mut &token_acc_data[..])?;
                require_keys_eq!(
                    recipient_token_acc.owner,
                    nft_owner,
                    StripVaultError::InvalidRecipientWallet
                );
            }
        }

        // Calculate share for this active Yield NFT
        let share_amount = calculate_yield_share(payout_to_distribute, yield_record.share_bps)?;

        if share_amount > 0 {
            match ctx.accounts.vault.payout_asset_type {
                PayoutAssetType::Usdc => {
                    let cpi_accounts = anchor_spl::token::Transfer {
                        from: ctx.accounts.vault_payout_source.to_account_info(),
                        to: recipient_payout_info.to_account_info(),
                        authority: ctx.accounts.vault.to_account_info(),
                    };
                    anchor_spl::token::transfer(
                        CpiContext::new_with_signer(
                            ctx.accounts.token_program.to_account_info(),
                            cpi_accounts,
                            vault_signer,
                        ),
                        share_amount,
                    )?;
                }
                PayoutAssetType::Sol => {
                    **ctx.accounts.vault.to_account_info().try_borrow_mut_lamports()? -=
                        share_amount;
                    **recipient_payout_info.try_borrow_mut_lamports()? += share_amount;
                }
            }
        }

        total_distributed_to_yield_holders = total_distributed_to_yield_holders
            .checked_add(share_amount)
            .ok_or(StripVaultError::MathOverflow)?;
        active_yield_holders_count += 1;

        // Increment payouts received count
        yield_record.payouts_received_count = yield_record
            .payouts_received_count
            .checked_add(1)
            .ok_or(StripVaultError::MathOverflow)?;

        // Check if this payout completed its PayoutCount expiry condition
        if yield_record.is_expired(clock.unix_timestamp) {
            yield_record.is_active = false;
            let vault_mut = &mut ctx.accounts.vault;
            vault_mut.total_yield_allocated_bps = vault_mut
                .total_yield_allocated_bps
                .saturating_sub(yield_record.share_bps);
        }

        yield_record.try_serialize(&mut &mut yield_record_data[..])?;
    }

    // 6. Transfer remainder (unallocated + newly expired shares) to Master Deed owner
    let master_deed_share = payout_to_distribute
        .checked_sub(total_distributed_to_yield_holders)
        .ok_or(StripVaultError::MathOverflow)?;

    if master_deed_share > 0 {
        match ctx.accounts.vault.payout_asset_type {
            PayoutAssetType::Usdc => {
                let cpi_accounts = anchor_spl::token::Transfer {
                    from: ctx.accounts.vault_payout_source.to_account_info(),
                    to: ctx.accounts.master_deed_payout_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                };
                anchor_spl::token::transfer(
                    CpiContext::new_with_signer(
                        ctx.accounts.token_program.to_account_info(),
                        cpi_accounts,
                        vault_signer,
                    ),
                    master_deed_share,
                )?;
            }
            PayoutAssetType::Sol => {
                **ctx.accounts.vault.to_account_info().try_borrow_mut_lamports()? -=
                    master_deed_share;
                **ctx.accounts.master_deed_payout_account.try_borrow_mut_lamports()? +=
                    master_deed_share;
            }
        }
    }

    emit!(PayoutDistributedEvent {
        vault: ctx.accounts.vault.key(),
        total_payout: payout_to_distribute,
        master_deed_share,
        yield_holders_count: active_yield_holders_count,
    });

    Ok(())
}
