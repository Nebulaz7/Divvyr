use anchor_lang::prelude::*;
use mpl_core::instructions::{BurnV1CpiBuilder, CreateV1CpiBuilder};
use mpl_core::types::{PermanentBurnDelegate, Plugin, PluginAuthority, PluginAuthorityPair};

pub const METAPLEX_CORE_ID: Pubkey = mpl_core::ID;

/// Helper function to mint a Metaplex Core Master Deed NFT directly to a recipient
pub fn mint_master_deed_nft<'info>(
    mpl_core_program: &AccountInfo<'info>,
    asset: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    recipient: &AccountInfo<'info>,
    name: String,
    uri: String,
) -> Result<()> {
    CreateV1CpiBuilder::new(mpl_core_program)
        .asset(asset)
        .payer(payer)
        .owner(Some(recipient))
        .name(name)
        .uri(uri)
        .invoke()?;

    Ok(())
}

/// Helper function to mint a Metaplex Core Yield NFT directly to a recipient,
/// attaching the PermanentBurnDelegate plugin with the Vault PDA as the authorized delegate.
pub fn mint_yield_nft<'info>(
    mpl_core_program: &AccountInfo<'info>,
    asset: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    recipient: &AccountInfo<'info>,
    vault_pda: &AccountInfo<'info>,
    name: String,
    uri: String,
) -> Result<()> {
    let plugins = vec![PluginAuthorityPair {
        plugin: Plugin::PermanentBurnDelegate(PermanentBurnDelegate {}),
        authority: Some(PluginAuthority::Address {
            address: vault_pda.key(),
        }),
    }];

    CreateV1CpiBuilder::new(mpl_core_program)
        .asset(asset)
        .payer(payer)
        .owner(Some(recipient))
        .name(name)
        .uri(uri)
        .plugins(plugins)
        .invoke()?;

    Ok(())
}

/// Helper function for the Vault PDA to burn a Yield NFT as the authorized PermanentBurnDelegate.
pub fn burn_yield_nft_as_delegate<'info>(
    mpl_core_program: &AccountInfo<'info>,
    asset: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    vault_pda: &AccountInfo<'info>,
    vault_signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    BurnV1CpiBuilder::new(mpl_core_program)
        .asset(asset)
        .payer(payer)
        .authority(Some(vault_pda))
        .invoke_signed(vault_signer_seeds)?;

    Ok(())
}

/// Helper function for the Master Deed owner to burn the Master Deed NFT upon principal redemption.
pub fn burn_master_deed_nft<'info>(
    mpl_core_program: &AccountInfo<'info>,
    asset: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
) -> Result<()> {
    BurnV1CpiBuilder::new(mpl_core_program)
        .asset(asset)
        .payer(payer)
        .authority(Some(owner))
        .invoke()?;

    Ok(())
}

