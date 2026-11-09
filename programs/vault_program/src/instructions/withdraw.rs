use anchor_lang::prelude::*;

use crate::{
    constants::{STATE, VAULT_SEED},
    error::ErrorCode,
    state::VaultState,
};

use anchor_lang::system_program::{transfer, Transfer};

#[derive(Accounts)]
pub struct Withdraw<'info> {
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

    pub system_program: Program<'info, System>,
}

impl<'info> Withdraw<'info> {
    pub fn withdraw(&mut self, amount: u64) -> Result<()> {
        let rent_exempt = Rent::get()?.minimum_balance(self.vault.data_len());

        let vault_balance = self.vault.lamports();

        require!(
            vault_balance >= rent_exempt,
            ErrorCode::InvalidWithdrawAmount
        );

        let available_balance = vault_balance - rent_exempt;

        require!(
            amount > 0 && amount <= available_balance,
            ErrorCode::InvalidWithdrawAmount
        );

        let user_key = self.user.key();
        let bump = [self.vault_state.vault_bump];

        let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, user_key.as_ref(), &bump]];

        let cpi_program = self.system_program.key();
        let cpi_account: Transfer<'_> = Transfer {
            from: self.vault.to_account_info(),
            to: self.user.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_account, signer_seeds);
        transfer(cpi_ctx, amount)
    }
}
