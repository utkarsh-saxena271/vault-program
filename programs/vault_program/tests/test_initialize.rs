use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    vault_program::{STATE, VAULT_SEED},
};

const DEPOSIT_LAMPORTS: u64 = 500_000_000;
const WITHDRAW_LAMPORTS: u64 = 100_000_000;

struct Ctx {
    svm: LiteSVM,
    user: Keypair,
    vault_state: Pubkey,
    vault: Pubkey,
}

fn derive(user: &Pubkey) -> (Pubkey, Pubkey) {
    let program_id = vault_program::id();
    let (vault_state, _) = Pubkey::find_program_address(&[STATE, user.as_ref()], &program_id);
    let (vault, _) = Pubkey::find_program_address(&[VAULT_SEED, user.as_ref()], &program_id);
    (vault_state, vault)
}

fn setup() -> Ctx {
    let program_id = vault_program::id();
    let user = Keypair::new();
    let (vault_state, vault) = derive(&user.pubkey());

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/vault_program.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&user.pubkey(), 2_000_000_000).unwrap();

    Ctx {
        svm,
        user,
        vault_state,
        vault,
    }
}

fn try_send(svm: &mut LiteSVM, payer: &Keypair, ix: Instruction) -> Result<(), String> {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}\n{}", e.err, e.meta.logs.join("\n")))
}

fn send(svm: &mut LiteSVM, payer: &Keypair, ix: Instruction) {
    try_send(svm, payer, ix).unwrap_or_else(|e| panic!("tx failed: {e}"));
}

fn init_ix(user: Pubkey, vault_state: Pubkey, vault: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        vault_program::id(),
        &vault_program::instruction::Initialize {}.data(),
        vault_program::accounts::Initialize {
            user,
            vault_state,
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn deposit_ix(user: Pubkey, vault_state: Pubkey, vault: Pubkey, amount: u64) -> Instruction {
    Instruction::new_with_bytes(
        vault_program::id(),
        &vault_program::instruction::Deposit { amount }.data(),
        vault_program::accounts::Deposit {
            user,
            vault_state,
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn withdraw_ix(user: Pubkey, vault_state: Pubkey, vault: Pubkey, amount: u64) -> Instruction {
    Instruction::new_with_bytes(
        vault_program::id(),
        &vault_program::instruction::Withdraw { amount }.data(),
        vault_program::accounts::Withdraw {
            user,
            vault_state,
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn close_ix(user: Pubkey, vault_state: Pubkey, vault: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        vault_program::id(),
        &vault_program::instruction::Close {}.data(),
        vault_program::accounts::Close {
            user,
            vault_state,
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

// initialize + deposit, returns the rent-exempt minimum of the vault
fn init_and_deposit(c: &mut Ctx, amount: u64) -> u64 {
    let ix = init_ix(c.user.pubkey(), c.vault_state, c.vault);
    send(&mut c.svm, &c.user, ix);
    let ix = deposit_ix(c.user.pubkey(), c.vault_state, c.vault, amount);
    send(&mut c.svm, &c.user, ix);
    c.svm.minimum_balance_for_rent_exemption(0)
}

#[test]
fn initialize_creates_state_and_funds_rent() {
    let mut c = setup();
    let ix = init_ix(c.user.pubkey(), c.vault_state, c.vault);
    send(&mut c.svm, &c.user, ix);

    let rent_exempt = c.svm.minimum_balance_for_rent_exemption(0);
    assert!(c.svm.get_account(&c.vault_state).is_some());
    assert_eq!(c.svm.get_balance(&c.vault).unwrap(), rent_exempt);
}

#[test]
fn initialize_twice_fails() {
    let mut c = setup();
    let ix = init_ix(c.user.pubkey(), c.vault_state, c.vault);
    send(&mut c.svm, &c.user, ix);

    let ix = init_ix(c.user.pubkey(), c.vault_state, c.vault);
    assert!(try_send(&mut c.svm, &c.user, ix).is_err());
}

#[test]
fn deposit_increases_vault_balance() {
    let mut c = setup();
    let rent_exempt = init_and_deposit(&mut c, DEPOSIT_LAMPORTS);
    assert_eq!(
        c.svm.get_balance(&c.vault).unwrap(),
        rent_exempt + DEPOSIT_LAMPORTS
    );
}

#[test]
fn deposit_zero_fails() {
    let mut c = setup();
    let ix = init_ix(c.user.pubkey(), c.vault_state, c.vault);
    send(&mut c.svm, &c.user, ix);
    let before = c.svm.get_balance(&c.vault).unwrap();

    let ix = deposit_ix(c.user.pubkey(), c.vault_state, c.vault, 0);
    assert!(try_send(&mut c.svm, &c.user, ix).is_err());
    assert_eq!(c.svm.get_balance(&c.vault).unwrap(), before);
}

#[test]
fn withdraw_reduces_vault_balance() {
    let mut c = setup();
    let rent_exempt = init_and_deposit(&mut c, DEPOSIT_LAMPORTS);

    let ix = withdraw_ix(c.user.pubkey(), c.vault_state, c.vault, WITHDRAW_LAMPORTS);
    send(&mut c.svm, &c.user, ix);

    assert_eq!(
        c.svm.get_balance(&c.vault).unwrap(),
        rent_exempt + DEPOSIT_LAMPORTS - WITHDRAW_LAMPORTS
    );
}

#[test]
fn withdraw_all_available_keeps_rent() {
    let mut c = setup();
    let rent_exempt = init_and_deposit(&mut c, DEPOSIT_LAMPORTS);

    let ix = withdraw_ix(c.user.pubkey(), c.vault_state, c.vault, DEPOSIT_LAMPORTS);
    send(&mut c.svm, &c.user, ix);

    assert_eq!(c.svm.get_balance(&c.vault).unwrap(), rent_exempt);
}

#[test]
fn withdraw_more_than_available_fails() {
    let mut c = setup();
    init_and_deposit(&mut c, DEPOSIT_LAMPORTS);
    let before = c.svm.get_balance(&c.vault).unwrap();

    let ix = withdraw_ix(
        c.user.pubkey(),
        c.vault_state,
        c.vault,
        DEPOSIT_LAMPORTS + 1,
    );
    assert!(try_send(&mut c.svm, &c.user, ix).is_err());
    assert_eq!(c.svm.get_balance(&c.vault).unwrap(), before);
}

#[test]
fn withdraw_zero_fails() {
    let mut c = setup();
    init_and_deposit(&mut c, DEPOSIT_LAMPORTS);
    let before = c.svm.get_balance(&c.vault).unwrap();

    let ix = withdraw_ix(c.user.pubkey(), c.vault_state, c.vault, 0);
    assert!(try_send(&mut c.svm, &c.user, ix).is_err());
    assert_eq!(c.svm.get_balance(&c.vault).unwrap(), before);
}

#[test]
fn other_user_cannot_withdraw_from_my_vault() {
    let mut c = setup();
    init_and_deposit(&mut c, DEPOSIT_LAMPORTS);
    let before = c.svm.get_balance(&c.vault).unwrap();

    let attacker = Keypair::new();
    c.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    // attacker signs, but points at the victim's PDAs
    let ix = withdraw_ix(attacker.pubkey(), c.vault_state, c.vault, WITHDRAW_LAMPORTS);
    assert!(try_send(&mut c.svm, &attacker, ix).is_err());
    assert_eq!(c.svm.get_balance(&c.vault).unwrap(), before);
}

#[test]
fn other_user_cannot_close_my_vault() {
    let mut c = setup();
    init_and_deposit(&mut c, DEPOSIT_LAMPORTS);
    let before = c.svm.get_balance(&c.vault).unwrap();

    let attacker = Keypair::new();
    c.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let ix = close_ix(attacker.pubkey(), c.vault_state, c.vault);
    assert!(try_send(&mut c.svm, &attacker, ix).is_err());
    assert_eq!(c.svm.get_balance(&c.vault).unwrap(), before);
    assert!(c.svm.get_account(&c.vault_state).is_some());
}

#[test]
fn close_returns_everything_and_closes_state() {
    let mut c = setup();
    init_and_deposit(&mut c, DEPOSIT_LAMPORTS);
    let user_before = c.svm.get_balance(&c.user.pubkey()).unwrap();

    let ix = close_ix(c.user.pubkey(), c.vault_state, c.vault);
    send(&mut c.svm, &c.user, ix);

    assert!(c.svm.get_account(&c.vault_state).is_none());
    assert_eq!(c.svm.get_balance(&c.vault).unwrap_or(0), 0);

    // user gets deposit + vault rent + state rent back, minus the tx fee
    let user_after = c.svm.get_balance(&c.user.pubkey()).unwrap();
    assert!(user_after > user_before + DEPOSIT_LAMPORTS);
}