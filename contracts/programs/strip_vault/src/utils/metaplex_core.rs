use anchor_lang::prelude::*;
use mpl_core::instructions::CreateV1CpiBuilder;

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
