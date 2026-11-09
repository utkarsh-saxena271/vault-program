use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Deposit amount must be greater than zero")]
    InvalidAmount,
    #[msg("Deposit amount must be less than or equal amount in vault")]
    InvalidWithdrawAmount,
}
