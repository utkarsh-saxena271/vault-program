use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Only the counter authority can update this counter")]
    Unauthorized,
    #[msg("Counter has reached the maximum value")]
    CounterOverflow,
    #[msg("Deposit amount must be greater than zero")]
    InvalidAmount,
    #[msg("Deposit amount must be less than or equal amount in vault")]
    InvalidWithdrawAmount,
}
