#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Env, IntoVal,
};

struct Setup {
    env: Env,
    admin: Address,
    usdc: Address,
    op_dest: Address,
    vault_id: Address,
}

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let op_dest = Address::generate(&env);

    let usdc_admin = Address::generate(&env);
    let tesouro_admin = Address::generate(&env);

    let usdc = env.register_stellar_asset_contract_v2(usdc_admin).address();
    let tesouro = env
        .register_stellar_asset_contract_v2(tesouro_admin)
        .address();

    // Constructor pattern: admin is set atomically at deploy time.
    let vault_id = env.register(ReserveVault, (admin.clone(),));

    let client = ReserveVaultClient::new(&env, &vault_id);

    // Admin populates allowlists with two test assets + one destination.
    client.add_approved_asset(&usdc);
    client.add_approved_asset(&tesouro);
    client.add_allowed_destination(&op_dest);

    Setup {
        env,
        admin,
        usdc,
        op_dest,
        vault_id,
    }
}

#[test]
fn constructor_sets_admin_and_defaults() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    assert_eq!(client.admin(), s.admin);
    assert!(!client.paused());
    assert_eq!(client.approved_assets().len(), 2);
    assert_eq!(client.allowed_destinations().len(), 1);
    assert!(client.is_destination_allowed(&s.op_dest));
    assert!(client.is_approved_asset(&s.usdc));
    assert!(client.pending_admin().is_none());
}

// ── asset allowlist ─────────────────────────────────────────────────────────

#[test]
fn add_remove_approved_asset() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let new_admin = Address::generate(&s.env);
    let other_asset = s
        .env
        .register_stellar_asset_contract_v2(new_admin)
        .address();
    client.add_approved_asset(&other_asset);
    assert!(client.is_approved_asset(&other_asset));
    assert_eq!(client.approved_assets().len(), 3);
    client.remove_approved_asset(&other_asset);
    assert!(!client.is_approved_asset(&other_asset));
    assert_eq!(client.approved_assets().len(), 2);
}

#[test]
#[should_panic]
fn add_duplicate_asset_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    client.add_approved_asset(&s.usdc);
}

#[test]
#[should_panic]
fn remove_asset_with_balance_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1);
    client.remove_approved_asset(&s.usdc);
}

// ── force_remove (escape hatch for sanctioned/frozen assets) ────────────────

#[test]
fn force_remove_approved_asset_succeeds_with_balance() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &42);

    client.force_remove_approved_asset(&s.usdc);
    assert!(!client.is_approved_asset(&s.usdc));
    // Balance still on-chain — the dust is abandoned, not destroyed.
    assert_eq!(client.balance(&s.usdc), 42);
}

#[test]
fn force_remove_emits_event_with_stranded_balance() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &7);

    client.force_remove_approved_asset(&s.usdc);

    let events = s.env.events().all().filter_by_contract(&s.vault_id);
    let expected_topics: soroban_sdk::Vec<soroban_sdk::Val> =
        soroban_sdk::vec![&s.env, symbol_short!("asset_frm").into_val(&s.env)];
    let expected_data: soroban_sdk::Val = (s.usdc.clone(), 7i128).into_val(&s.env);
    let expected: soroban_sdk::Vec<(
        Address,
        soroban_sdk::Vec<soroban_sdk::Val>,
        soroban_sdk::Val,
    )> = soroban_sdk::vec![&s.env, (s.vault_id.clone(), expected_topics, expected_data)];
    assert_eq!(events, expected);
}

#[test]
#[should_panic]
fn force_remove_unknown_asset_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let issuer = Address::generate(&s.env);
    let unknown = s.env.register_stellar_asset_contract_v2(issuer).address();
    client.force_remove_approved_asset(&unknown);
}

// ── destination allowlist ───────────────────────────────────────────────────

#[test]
fn add_remove_destination() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let new_dest = Address::generate(&s.env);
    client.add_allowed_destination(&new_dest);
    assert!(client.is_destination_allowed(&new_dest));
    assert_eq!(client.allowed_destinations().len(), 2);
    client.remove_allowed_destination(&new_dest);
    assert!(!client.is_destination_allowed(&new_dest));
}

#[test]
#[should_panic]
fn add_duplicate_destination_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    client.add_allowed_destination(&s.op_dest);
}

// ── withdraw ────────────────────────────────────────────────────────────────

#[test]
fn withdraw_to_whitelisted_destination_succeeds() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);

    let token = TokenClient::new(&s.env, &s.usdc);
    let bal_before = token.balance(&s.op_dest);

    let amount = 50_000_000_000i128; // 5k USDC
    let ref_hash = BytesN::from_array(&s.env, &[7u8; 32]);
    client.withdraw(&s.usdc, &amount, &s.op_dest, &ref_hash);

    assert_eq!(token.balance(&s.op_dest) - bal_before, amount);
}

#[test]
#[should_panic]
fn withdraw_to_non_whitelisted_destination_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);

    let attacker = Address::generate(&s.env);
    let ref_hash = BytesN::from_array(&s.env, &[1u8; 32]);
    client.withdraw(&s.usdc, &50_000_000_000, &attacker, &ref_hash);
}

#[test]
#[should_panic]
fn withdraw_of_unapproved_asset_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let unknown_asset_admin = Address::generate(&s.env);
    let unknown_asset = s
        .env
        .register_stellar_asset_contract_v2(unknown_asset_admin)
        .address();
    let ref_hash = BytesN::from_array(&s.env, &[2u8; 32]);
    client.withdraw(&unknown_asset, &50_000_000_000, &s.op_dest, &ref_hash);
}

#[test]
#[should_panic]
fn withdraw_zero_amount_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let ref_hash = BytesN::from_array(&s.env, &[3u8; 32]);
    client.withdraw(&s.usdc, &0, &s.op_dest, &ref_hash);
}

#[test]
fn withdraw_blocked_when_paused() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);

    client.set_paused(&true);
    assert!(client.paused());

    let ref_hash = BytesN::from_array(&s.env, &[4u8; 32]);
    let result = client.try_withdraw(&s.usdc, &50_000_000_000, &s.op_dest, &ref_hash);
    assert!(result.is_err());
}

#[test]
fn unpause_resumes_withdrawals() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);

    client.set_paused(&true);
    client.set_paused(&false);

    let ref_hash = BytesN::from_array(&s.env, &[5u8; 32]);
    client.withdraw(&s.usdc, &50_000_000_000, &s.op_dest, &ref_hash);

    let token = TokenClient::new(&s.env, &s.usdc);
    assert_eq!(token.balance(&s.op_dest), 50_000_000_000);
}

// ── admin handover with timelock ────────────────────────────────────────────

#[test]
fn two_step_admin_handover_with_expiry() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let new_admin = Address::generate(&s.env);
    let current = s.env.ledger().sequence();
    let live_until = current + 1_000;

    client.propose_admin(&new_admin, &live_until);
    let pending = client.pending_admin().expect("pending admin set");
    assert_eq!(pending.address, new_admin);
    assert_eq!(pending.live_until_ledger, live_until);

    client.accept_admin();
    assert_eq!(client.admin(), new_admin);
    assert!(client.pending_admin().is_none());
}

#[test]
fn propose_admin_with_zero_cancels_pending() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let new_admin = Address::generate(&s.env);
    let live_until = s.env.ledger().sequence() + 1_000;

    client.propose_admin(&new_admin, &live_until);
    assert!(client.pending_admin().is_some());

    client.propose_admin(&new_admin, &0);
    assert!(client.pending_admin().is_none());
}

#[test]
#[should_panic]
fn propose_admin_with_past_ledger_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let new_admin = Address::generate(&s.env);
    s.env.ledger().with_mut(|li| li.sequence_number = 500);
    // live_until before current sequence → InvalidExpiry
    client.propose_admin(&new_admin, &499);
}

#[test]
fn accept_admin_after_expiry_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let new_admin = Address::generate(&s.env);
    let live_until = s.env.ledger().sequence() + 100;

    client.propose_admin(&new_admin, &live_until);

    // Advance past the deadline; the temporary entry's TTL would also expire
    // but the in-contract re-check is what we're asserting here.
    s.env
        .ledger()
        .with_mut(|li| li.sequence_number = live_until + 1);

    let result = client.try_accept_admin();
    assert!(result.is_err());
    // Original admin still in place.
    assert_eq!(client.admin(), s.admin);
}

#[test]
#[should_panic]
fn accept_admin_without_pending_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    client.accept_admin();
}

// ── allowlist capacity ──────────────────────────────────────────────────────

#[test]
#[should_panic]
fn add_approved_asset_at_capacity_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    // setup already added 2; add 6 more to hit MAX_APPROVED_ASSETS = 8, then 1 more must panic.
    for _ in 0..6 {
        let issuer = Address::generate(&s.env);
        let asset = s.env.register_stellar_asset_contract_v2(issuer).address();
        client.add_approved_asset(&asset);
    }
    let overflow_issuer = Address::generate(&s.env);
    let overflow_asset = s
        .env
        .register_stellar_asset_contract_v2(overflow_issuer)
        .address();
    client.add_approved_asset(&overflow_asset);
}

#[test]
#[should_panic]
fn add_allowed_destination_at_capacity_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    // setup already added 1; add 63 more to hit MAX_ALLOWED_DESTINATIONS = 64, then 1 more must panic.
    for _ in 0..63 {
        let dest = Address::generate(&s.env);
        client.add_allowed_destination(&dest);
    }
    let overflow = Address::generate(&s.env);
    client.add_allowed_destination(&overflow);
}

#[test]
#[should_panic]
fn remove_unknown_asset_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let issuer = Address::generate(&s.env);
    let unknown = s.env.register_stellar_asset_contract_v2(issuer).address();
    client.remove_approved_asset(&unknown);
}

#[test]
#[should_panic]
fn remove_unknown_destination_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let unknown = Address::generate(&s.env);
    client.remove_allowed_destination(&unknown);
}

// ── events ──────────────────────────────────────────────────────────────────

#[test]
fn withdraw_emits_event_with_correct_topics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);

    let amount = 50_000_000_000i128;
    let ref_hash = BytesN::from_array(&s.env, &[9u8; 32]);
    client.withdraw(&s.usdc, &amount, &s.op_dest, &ref_hash);

    let events = s.env.events().all().filter_by_contract(&s.vault_id);
    let expected_topics: soroban_sdk::Vec<soroban_sdk::Val> = soroban_sdk::vec![
        &s.env,
        symbol_short!("withdraw").into_val(&s.env),
        ref_hash.clone().into_val(&s.env),
    ];
    let expected_data: soroban_sdk::Val =
        (s.usdc.clone(), amount, s.op_dest.clone()).into_val(&s.env);
    let expected: soroban_sdk::Vec<(
        Address,
        soroban_sdk::Vec<soroban_sdk::Val>,
        soroban_sdk::Val,
    )> = soroban_sdk::vec![&s.env, (s.vault_id.clone(), expected_topics, expected_data)];
    assert_eq!(events, expected);
}

// ── balance view ────────────────────────────────────────────────────────────

#[test]
fn balance_view_reads_sep41() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);

    assert_eq!(client.balance(&s.usdc), 0);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);
    assert_eq!(client.balance(&s.usdc), 1_000_000_000_000);
}
