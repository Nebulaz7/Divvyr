use crate::errors::StripVaultError;
use crate::events::PrincipalRedeemedEvent;
use crate::state::Vault;
use crate::utils::metaplex_core::burn_master_deed_nft;
use crate::utils::scaled_ui::MINT_DECIMALS_OFFSET;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Token2022, TokenAccount};
use mpl_core::accounts::BaseAssetV1;

#[derive(Accounts)]
pub struct OwnerRedeemPrincipal<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        close = owner,
        seeds = [Vault::SEED_PREFIX, vault.stock_mint.as_ref(), &vault.vault_seed.to_le_bytes()],
        bump = vault.vault_bump,
    )]
    pub vault: Account<'info, Vault>,

    /// CHECK: Validated against vault.stock_mint
    #[account(address = vault.stock_mint)]
    pub stock_mint: AccountInfo<'info>,

    #[account(
        mut,
        address = vault.stock_vault_ata,
        token::mint = stock_mint,
        token::authority = vault,
        token::token_program = token_program_2022,
    )]
    pub stock_vault_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = stock_mint,
        token::authority = owner,
        token::token_program = token_program_2022,
    )]
    pub owner_stock_ata: InterfaceAccount<'info, TokenAccount>,

    /// Metaplex Core Master Deed Asset account to be burned
    /// CHECK: Validated against vault.master_deed_asset and mpl_core::ID
    #[account(mut, address = vault.master_deed_asset)]
    pub master_deed_asset: AccountInfo<'info>,

    pub token_program_2022: Program<'info, Token2022>,
    /// CHECK: Metaplex Core program verified by address
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<OwnerRedeemPrincipal>) -> Result<()> {
    let vault = &ctx.accounts.vault;

    // 1. Verify all yield positions have either expired or been bought out
    require!(
        vault.total_yield_allocated_bps == 0,
        StripVaultError::ActiveYieldPositionsRemain
    );

    // 2. Verify caller is current owner of Master Deed NFT
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

    // 3. Burn Master Deed NFT via Metaplex Core CPI (authorized by owner)
    burn_master_deed_nft(
        &ctx.accounts.mpl_core_program,
        &ctx.accounts.master_deed_asset,
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
    )?;

    // 4. Transfer all remaining stock tokens from vault ATA to owner ATA
    let remaining_tokens = ctx.accounts.stock_vault_ata.amount;
    let vault_seed_bytes = vault.vault_seed.to_le_bytes();
    let vault_seeds: &[&[u8]] = &[
        Vault::SEED_PREFIX,
        vault.stock_mint.as_ref(),
        &vault_seed_bytes,
        &[vault.vault_bump],
    ];
    let vault_signer = &[vault_seeds];

    if remaining_tokens > 0 {
        let mint_data = ctx.accounts.stock_mint.try_borrow_data()?;
        require!(
            mint_data.len() > MINT_DECIMALS_OFFSET,
            StripVaultError::InvalidScaledUiAmountConfig
        );
        let decimals = mint_data[MINT_DECIMALS_OFFSET];
        drop(mint_data);

        let transfer_accounts = anchor_spl::token_2022::TransferChecked {
            from: ctx.accounts.stock_vault_ata.to_account_info(),
            mint: ctx.accounts.stock_mint.to_account_info(),
            to: ctx.accounts.owner_stock_ata.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };

        anchor_spl::token_2022::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program_2022.to_account_info(),
                transfer_accounts,
                vault_signer,
            ),
            remaining_tokens,
            decimals,
        )?;
    }

    // 5. Close stock_vault_ata and send rent to owner
    let close_accounts = anchor_spl::token_2022::CloseAccount {
        account: ctx.accounts.stock_vault_ata.to_account_info(),
        destination: ctx.accounts.owner.to_account_info(),
        authority: ctx.accounts.vault.to_account_info(),
    };

    anchor_spl::token_2022::close_account(
        CpiContext::new_with_signer(
            ctx.accounts.token_program_2022.to_account_info(),
            close_accounts,
            vault_signer,
        ),
    )?;

    emit!(PrincipalRedeemedEvent {
        vault: vault.key(),
        owner: ctx.accounts.owner.key(),
        stock_mint: vault.stock_mint,
        stock_shares_returned: remaining_tokens,
    });

    Ok(())
}
