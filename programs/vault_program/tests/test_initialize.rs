use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    vault_program::{STATE, VAULT_SEED},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

const DEPOSIT_LAMPORTS: u64 = 500_000_000;
const WITHDRAW_LAMPORTS: u64 = 100_000_000;

fn send(svm: &mut LiteSVM, payer: &Keypair, ix: Instruction) {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).unwrap_or_else(|err| {
        panic!("tx failed: {err:?}\nlogs: {logs:#?}", logs = err.meta.logs);
    });
}

#[test]
fn test() {
    let program_id = vault_program::id();
    let user = Keypair::new();
    let (vault_state, _) =
        Pubkey::find_program_address(&[STATE, user.pubkey().as_ref()], &program_id);
    let (vault, _) =
        Pubkey::find_program_address(&[VAULT_SEED, user.pubkey().as_ref()], &program_id);

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/vault_program.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&user.pubkey(), 2_000_000_000).unwrap();

    send(
        &mut svm,
        &user,
        Instruction::new_with_bytes(
            program_id,
            &vault_program::instruction::Initialize {}.data(),
            vault_program::accounts::Initialize {
                user: user.pubkey(),
                vault_state,
                vault,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        ),
    );

    let rent_exempt = svm.minimum_balance_for_rent_exemption(0);
    assert!(
        svm.get_account(&vault_state).is_some(),
        "vault_state should exist after initialize"
    );
    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt,
        "vault should be rent-exempt after initialize"
    );

    send(
        &mut svm,
        &user,
        Instruction::new_with_bytes(
            program_id,
            &vault_program::instruction::Deposit {
                amount: DEPOSIT_LAMPORTS,
            }
            .data(),
            vault_program::accounts::Deposit {
                user: user.pubkey(),
                vault_state,
                vault,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        ),
    );

    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt + DEPOSIT_LAMPORTS,
        "vault should increase by the deposit"
    );

    send(
        &mut svm,
        &user,
        Instruction::new_with_bytes(
            program_id,
            &vault_program::instruction::Withdraw {
                amount: WITHDRAW_LAMPORTS,
            }
            .data(),
            vault_program::accounts::Withdraw {
                user: user.pubkey(),
                vault_state,
                vault,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        ),
    );

    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt + DEPOSIT_LAMPORTS - WITHDRAW_LAMPORTS,
        "vault should decrease by the withdraw"
    );

    send(
        &mut svm,
        &user,
        Instruction::new_with_bytes(
            program_id,
            &vault_program::instruction::Close {}.data(),
            vault_program::accounts::Close {
                user: user.pubkey(),
                vault_state,
                vault,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        ),
    );

    assert!(
        svm.get_account(&vault_state).is_none(),
        "vault_state should be closed"
    );
    assert_eq!(
        svm.get_balance(&vault).unwrap_or(0),
        0,
        "vault should be empty"
    );
}