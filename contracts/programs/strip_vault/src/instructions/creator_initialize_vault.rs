use crate::errors::StripVaultError;
use crate::events::VaultInitializedEvent;
use crate::state::{PayoutAssetType, Vault};
use crate::utils::metaplex_core::mint_master_deed_nft;
use crate::utils::scaled_ui::unpack_scaled_ui_mint;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Token2022, TokenAccount};

#[derive(Accounts)]
#[instruction(
    amount: u64,
    payout_asset_type: PayoutAssetType,
    vault_seed: u64,
    name: String,
    uri: String
)]
pub struct CreatorInitializeVault<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    /// CHECK: Validated via unpack_scaled_ui_mint in handler
    pub stock_mint: AccountInfo<'info>,

    #[account(
        mut,
        token::mint = stock_mint,
        token::authority = creator,
        token::token_program = token_program_2022,
    )]
    pub creator_stock_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = creator,
        space = 8 + Vault::INIT_SPACE,
        seeds = [Vault::SEED_PREFIX, stock_mint.key().as_ref(), &vault_seed.to_le_bytes()],
        bump
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        init,
        payer = creator,
        associated_token::mint = stock_mint,
        associated_token::authority = vault,
        associated_token::token_program = token_program_2022,
    )]
    pub stock_vault_ata: InterfaceAccount<'info, TokenAccount>,

    /// Metaplex Core Master Deed Asset keypair to be created and minted
    #[account(mut)]
    pub master_deed_asset: Signer<'info>,

    /// CHECK: Payout holding account (USDC ATA or Vault PDA for SOL)
    pub payout_holding_ata: AccountInfo<'info>,

    pub token_program_2022: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    /// CHECK: Metaplex Core program verified by address
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreatorInitializeVault>,
    amount: u64,
    payout_asset_type: PayoutAssetType,
    vault_seed: u64,
    name: String,
    uri: String,
) -> Result<()> {
    require!(amount > 0, StripVaultError::MathOverflow);

    // 1. Unpack Token-2022 ScaledUiAmountConfig extension and read active multiplier
    let decimals: u8;
    let initial_multiplier: u64;
    {
        let mint_data = ctx.accounts.stock_mint.try_borrow_data()?;
        let (mint_decimals, config) = unpack_scaled_ui_mint(&mint_data)?;
        decimals = mint_decimals;

        let clock = Clock::get()?;
        initial_multiplier = config.get_effective_multiplier_fixed(clock.unix_timestamp)?;
    }

    // 2. Transfer stock tokens into vault ATA via Token-2022 CPI
    let transfer_accounts = anchor_spl::token_2022::TransferChecked {
        from: ctx.accounts.creator_stock_ata.to_account_info(),
        mint: ctx.accounts.stock_mint.to_account_info(),
        to: ctx.accounts.stock_vault_ata.to_account_info(),
        authority: ctx.accounts.creator.to_account_info(),
    };
    anchor_spl::token_2022::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program_2022.to_account_info(),
            transfer_accounts,
        ),
        amount,
        decimals,
    )?;

    // 3. Mint Master Deed NFT to creator via Metaplex Core CPI with dynamic name and uri
    mint_master_deed_nft(
        &ctx.accounts.mpl_core_program,
        &ctx.accounts.master_deed_asset,
        &ctx.accounts.creator.to_account_info(),
        &ctx.accounts.creator.to_account_info(),
        name,
        uri,
    )?;

    // 4. Initialize Vault state
    let vault = &mut ctx.accounts.vault;
    vault.stock_mint = ctx.accounts.stock_mint.key();
    vault.stock_vault_ata = ctx.accounts.stock_vault_ata.key();
    vault.master_deed_asset = ctx.accounts.master_deed_asset.key();
    vault.payout_holding_ata = ctx.accounts.payout_holding_ata.key();
    vault.total_raw_shares = amount;
    vault.last_multiplier = initial_multiplier;
    vault.pending_harvest_amount = 0;
    vault.total_yield_allocated_bps = 0;
    vault.payout_asset_type = payout_asset_type;
    vault.vault_bump = ctx.bumps.vault;
    vault.vault_seed = vault_seed;
    vault._reserved = [0u8; 64];

    emit!(VaultInitializedEvent {
        vault: vault.key(),
        stock_mint: vault.stock_mint,
        master_deed_asset: vault.master_deed_asset,
        initial_shares: amount,
        initial_multiplier,
    });

    Ok(())
}
