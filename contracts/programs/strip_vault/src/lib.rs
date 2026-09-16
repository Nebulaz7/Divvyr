use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

pub use errors::*;
pub use events::*;
pub use state::*;
pub use utils::*;

declare_id!("Divvyr1111111111111111111111111111111111111");

#[program]
pub mod strip_vault {
    use super::*;

    // Placeholder entrypoint for Part 1 compilation verification
    pub fn ping(_ctx: Context<Ping>) -> Result<()> {
        msg!("Divvyr strip_vault program live!");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Ping<'info> {
    pub signer: Signer<'info>,
}
