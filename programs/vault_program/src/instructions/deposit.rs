use anchor_lang::prelude::*;

use crate::{
    constants::{STATE, VAULT_SEED}, state::VaultState, error::ErrorCode
};

use anchor_lang::{
    system_program::{transfer, Transfer}
};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [STATE, user.key().as_ref()],
        bump = vault_state.state_bump
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        seeds = [VAULT_SEED, user.key().as_ref()],
        bump = vault_state.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>
}

impl <'info> Deposit<'info> {
    pub fn deposit(&mut self, amount: u64) -> Result<()>{
        require!(amount > 0, ErrorCode::InvalidAmount);

        let cpi_program = self.system_program.key();
        let cpi_account: Transfer<'_> = Transfer {
            from: self.user.to_account_info(),
            to: self.vault.to_account_info()
        };

        let cpi_ctx = CpiContext::new(cpi_program, cpi_account);
        transfer(cpi_ctx, amount)
    }
}