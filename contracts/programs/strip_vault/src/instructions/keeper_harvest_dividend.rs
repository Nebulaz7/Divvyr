use crate::errors::StripVaultError;
use crate::events::DividendHarvestedEvent;
use crate::state::Vault;
use crate::utils::math::calculate_harvest_amount;
use crate::utils::scaled_ui::unpack_scaled_ui_mint;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Token2022, TokenAccount};

#[derive(Accounts)]
pub struct KeeperHarvestDividend<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [Vault::SEED_PREFIX, vault.stock_mint.as_ref(), &vault.vault_seed.to_le_bytes()],
        bump = vault.vault_bump,
    )]
    pub vault: Account<'info, Vault>,

    /// CHECK: Validated against vault.stock_mint and unpack_scaled_ui_mint
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

    /// CHECK: Canonical PDA authority that owns the pending distribution ATA
    #[account(
        seeds = [b"pending_authority", vault.key().as_ref()],
        bump,
    )]
    pub pending_authority: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = keeper,
        associated_token::mint = stock_mint,
        associated_token::authority = pending_authority,
        associated_token::token_program = token_program_2022,
    )]
    pub pending_distribution_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program_2022: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<KeeperHarvestDividend>) -> Result<()> {
    // 1. Unpack ScaledUiAmountConfig from Token-2022 mint and determine active multiplier
    let mint_data = ctx.accounts.stock_mint.try_borrow_data()?;
    let (decimals, config) = unpack_scaled_ui_mint(&mint_data)?;

    let clock = Clock::get()?;
    let active_multiplier = config.get_effective_multiplier_fixed(clock.unix_timestamp)?;
    drop(mint_data);

    let vault = &ctx.accounts.vault;
    let old_multiplier = vault.last_multiplier;

    // 2. Validate that multiplier has increased
    require!(
        active_multiplier > old_multiplier,
        StripVaultError::NoDividendDelta
    );

    // 3. Calculate raw delta tokens to harvest
    let current_raw_balance = ctx.accounts.stock_vault_ata.amount;
    let harvest_amount = calculate_harvest_amount(
        current_raw_balance,
        old_multiplier,
        active_multiplier,
    )?;

    require!(harvest_amount > 0, StripVaultError::NoDividendDelta);

    // 4. Transfer delta tokens from stock_vault_ata to pending_distribution_ata signed by Vault PDA
    let vault_seed_bytes = vault.vault_seed.to_le_bytes();
    let vault_seeds: &[&[u8]] = &[
        Vault::SEED_PREFIX,
        vault.stock_mint.as_ref(),
        &vault_seed_bytes,
        &[vault.vault_bump],
    ];
    let signer_seeds = &[vault_seeds];

    let transfer_accounts = anchor_spl::token_2022::TransferChecked {
        from: ctx.accounts.stock_vault_ata.to_account_info(),
        mint: ctx.accounts.stock_mint.to_account_info(),
        to: ctx.accounts.pending_distribution_ata.to_account_info(),
        authority: ctx.accounts.vault.to_account_info(),
    };

    anchor_spl::token_2022::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program_2022.to_account_info(),
            transfer_accounts,
            signer_seeds,
        ),
        harvest_amount,
        decimals,
    )?;

    // 5. Update Vault state
    let vault_mut = &mut ctx.accounts.vault;
    vault_mut.last_multiplier = active_multiplier;
    vault_mut.pending_harvest_amount = vault_mut
        .pending_harvest_amount
        .checked_add(harvest_amount)
        .ok_or(StripVaultError::MathOverflow)?;

    let remaining_shares = current_raw_balance
        .checked_sub(harvest_amount)
        .ok_or(StripVaultError::MathOverflow)?;
    vault_mut.total_raw_shares = remaining_shares;

    emit!(DividendHarvestedEvent {
        vault: vault_mut.key(),
        old_multiplier,
        new_multiplier: active_multiplier,
        harvested_tokens: harvest_amount,
    });

    Ok(())
}
