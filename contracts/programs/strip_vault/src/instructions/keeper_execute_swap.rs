use crate::errors::StripVaultError;
use crate::events::SwapExecutedEvent;
use crate::state::{PayoutAssetType, Vault};
use crate::utils::math::calculate_simulated_swap_output;
use crate::utils::scaled_ui::MINT_DECIMALS_OFFSET;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Token2022, TokenAccount, TokenInterface};

#[derive(Accounts)]
#[instruction(custom_rate: Option<u64>)]
pub struct KeeperExecuteSwap<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [Vault::SEED_PREFIX, vault.stock_mint.as_ref(), &vault.vault_seed.to_le_bytes()],
        bump = vault.vault_bump,
    )]
    pub vault: Account<'info, Vault>,

    /// CHECK: Validated against vault.stock_mint
    #[account(address = vault.stock_mint)]
    pub stock_mint: AccountInfo<'info>,

    /// CHECK: Canonical PDA authority that owns the pending distribution ATA
    #[account(
        seeds = [b"pending_authority", vault.key().as_ref()],
        bump,
    )]
    pub pending_authority: AccountInfo<'info>,

    #[account(
        mut,
        associated_token::mint = stock_mint,
        associated_token::authority = pending_authority,
        associated_token::token_program = token_program_2022,
    )]
    pub pending_distribution_ata: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Canonical Swap Reserve PDA authority
    #[account(
        mut,
        seeds = [b"swap_reserve", vault.key().as_ref()],
        bump,
    )]
    pub swap_reserve_authority: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = keeper,
        associated_token::mint = stock_mint,
        associated_token::authority = swap_reserve_authority,
        associated_token::token_program = token_program_2022,
    )]
    pub swap_reserve_stock_ata: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Payout account to receive swapped USDC (vault.payout_holding_ata) or SOL (vault PDA)
    #[account(mut)]
    pub vault_payout_account: AccountInfo<'info>,

    /// CHECK: Payout source in reserve (USDC ATA or swap_reserve_authority PDA for SOL)
    #[account(mut)]
    pub swap_reserve_payout_account: AccountInfo<'info>,

    pub token_program_2022: Program<'info, Token2022>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<KeeperExecuteSwap>, custom_rate: Option<u64>) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let pending_amount = vault.pending_harvest_amount;

    require!(pending_amount > 0, StripVaultError::NoPendingHarvest);

    // 1. Read stock mint decimals
    let mint_data = ctx.accounts.stock_mint.try_borrow_data()?;
    require!(
        mint_data.len() > MINT_DECIMALS_OFFSET,
        StripVaultError::InvalidScaledUiAmountConfig
    );
    let decimals = mint_data[MINT_DECIMALS_OFFSET];
    drop(mint_data);

    // 2. Calculate swap output amount based on deterministic oracle rate
    let payout_amount = calculate_simulated_swap_output(
        pending_amount,
        decimals,
        vault.payout_asset_type,
        custom_rate,
    )?;

    // 3. Transfer pending mock stock from pending_distribution_ata to swap_reserve_stock_ata
    // signed by pending_authority PDA
    let vault_key = vault.key();
    let pending_authority_bump = ctx.bumps.pending_authority;
    let pending_seeds: &[&[u8]] = &[
        b"pending_authority",
        vault_key.as_ref(),
        &[pending_authority_bump],
    ];
    let pending_signer = &[pending_seeds];

    let transfer_stock_accounts = anchor_spl::token_2022::TransferChecked {
        from: ctx.accounts.pending_distribution_ata.to_account_info(),
        mint: ctx.accounts.stock_mint.to_account_info(),
        to: ctx.accounts.swap_reserve_stock_ata.to_account_info(),
        authority: ctx.accounts.pending_authority.to_account_info(),
    };

    anchor_spl::token_2022::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program_2022.to_account_info(),
            transfer_stock_accounts,
            pending_signer,
        ),
        pending_amount,
        decimals,
    )?;

    // 4. Release equivalent USDC or SOL from reserve to vault payout account
    let swap_reserve_bump = ctx.bumps.swap_reserve_authority;
    let reserve_seeds: &[&[u8]] = &[
        b"swap_reserve",
        vault_key.as_ref(),
        &[swap_reserve_bump],
    ];
    let reserve_signer = &[reserve_seeds];

    match vault.payout_asset_type {
        PayoutAssetType::Usdc => {
            // Validate destination account matches vault.payout_holding_ata
            require_keys_eq!(
                ctx.accounts.vault_payout_account.key(),
                vault.payout_holding_ata,
                StripVaultError::InvalidRecipientWallet
            );

            // Execute SPL Token transfer from reserve payout ATA to vault payout ATA
            let transfer_usdc_accounts = anchor_spl::token::Transfer {
                from: ctx.accounts.swap_reserve_payout_account.to_account_info(),
                to: ctx.accounts.vault_payout_account.to_account_info(),
                authority: ctx.accounts.swap_reserve_authority.to_account_info(),
            };

            anchor_spl::token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    transfer_usdc_accounts,
                    reserve_signer,
                ),
                payout_amount,
            )?;
        }
        PayoutAssetType::Sol => {
            // Validate reserve has sufficient lamports
            let reserve_lamports = ctx.accounts.swap_reserve_authority.lamports();
            require!(
                reserve_lamports >= payout_amount,
                StripVaultError::InsufficientReserveLiquidity
            );

            // Transfer lamports from swap_reserve_authority PDA to vault PDA
            **ctx.accounts.swap_reserve_authority.try_borrow_mut_lamports()? -= payout_amount;
            **ctx.accounts.vault_payout_account.try_borrow_mut_lamports()? += payout_amount;
        }
    }

    // 5. Reset pending harvest amount
    let vault_mut = &mut ctx.accounts.vault;
    vault_mut.pending_harvest_amount = 0;

    emit!(SwapExecutedEvent {
        vault: vault_mut.key(),
        stock_amount_in: pending_amount,
        payout_amount_out: payout_amount,
    });

    Ok(())
}
