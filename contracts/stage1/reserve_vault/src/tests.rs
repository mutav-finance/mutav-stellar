#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Env,
};

const PAY_DEFAULT_MAX_ITEM_VALUE: i128 = 150_000_000_000; // 15k USDC (7 decimals)
const PAY_DEFAULT_TIMELOCK_SECS: u64 = 86_400; // 24h
const MAX_ITEMS_PER_BATCH: u32 = 50;
const MAX_PENDING_PROPOSALS: u32 = 100;
const MAX_RATE_STALENESS_SECS: u64 = 86_400;

struct Setup {
    env: Env,
    admin: Address,
    operator: Address,
    usdc: Address,
    tesouro: Address,
    op_dest: Address,
    vault_id: Address,
}

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    let op_dest = Address::generate(&env);

    let usdc_admin = Address::generate(&env);
    let tesouro_admin = Address::generate(&env);

    let usdc = env.register_stellar_asset_contract_v2(usdc_admin).address();
    let tesouro = env
        .register_stellar_asset_contract_v2(tesouro_admin)
        .address();

    let vault_id = env.register(ReserveVault, ());

    let mut approved = Vec::new(&env);
    approved.push_back(usdc.clone());
    approved.push_back(tesouro.clone());

    let mut dests = Vec::new(&env);
    dests.push_back(op_dest.clone());

    ReserveVaultClient::new(&env, &vault_id).initialize(
        &admin,
        &operator,
        &approved,
        &usdc,
        &dests,
        &PAY_DEFAULT_MAX_ITEM_VALUE,
        &PAY_DEFAULT_TIMELOCK_SECS,
        &MAX_ITEMS_PER_BATCH,
        &MAX_PENDING_PROPOSALS,
        &MAX_RATE_STALENESS_SECS,
    );

    Setup {
        env,
        admin,
        operator,
        usdc,
        tesouro,
        op_dest,
        vault_id,
    }
}

#[test]
fn initialize_sets_storage_correctly() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    assert_eq!(client.admin(), s.admin);
    assert_eq!(client.operator(), s.operator);
    assert!(!client.paused());
    assert_eq!(client.denomination_asset(), s.usdc);
    assert_eq!(client.approved_assets().len(), 2);
    assert_eq!(client.allowed_destinations().len(), 1);
    assert!(client.is_destination_allowed(&s.op_dest));
}

#[test]
#[should_panic]
fn initialize_rejects_denomination_not_in_approved() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    let other_admin = Address::generate(&env);
    let usdc = env
        .register_stellar_asset_contract_v2(other_admin.clone())
        .address();
    let denom = Address::generate(&env); // not in approved

    let vault_id = env.register(ReserveVault, ());

    let mut approved = Vec::new(&env);
    approved.push_back(usdc);

    ReserveVaultClient::new(&env, &vault_id).initialize(
        &admin,
        &operator,
        &approved,
        &denom,
        &Vec::new(&env),
        &PAY_DEFAULT_MAX_ITEM_VALUE,
        &PAY_DEFAULT_TIMELOCK_SECS,
        &MAX_ITEMS_PER_BATCH,
        &MAX_PENDING_PROPOSALS,
        &MAX_RATE_STALENESS_SECS,
    );
}

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
fn remove_denomination_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    client.remove_approved_asset(&s.usdc);
}

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
fn record_capital_receipt_emits_event() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let source = Address::generate(&s.env);
    let hash = BytesN::from_array(&s.env, &[1u8; 32]);
    client.record_capital_receipt(&source, &s.usdc, &10_000_000_000, &hash);
    // No panic = success.
}

#[test]
#[should_panic]
fn record_capital_receipt_replay_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let source = Address::generate(&s.env);
    let hash = BytesN::from_array(&s.env, &[1u8; 32]);
    client.record_capital_receipt(&source, &s.usdc, &10_000_000_000, &hash);
    client.record_capital_receipt(&source, &s.usdc, &10_000_000_000, &hash);
}

#[test]
fn pay_default_full_lifecycle_with_denomination_asset() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);

    // Mint USDC to the vault for the payout.
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000); // 100k USDC

    let agency = Address::generate(&s.env);
    let guarantee_hash = BytesN::from_array(&s.env, &[7u8; 32]);
    let amount = 50_000_000_000i128; // 5k USDC

    let item = PayDefaultItem {
        asset: s.usdc.clone(),
        amount,
        destination: agency.clone(),
        guarantee_contract_hash: guarantee_hash,
        covered_month: 202609,
    };
    let mut items = Vec::new(&s.env);
    items.push_back(item);

    let id = client.propose_pay_default(&items);
    assert_eq!(id, 0);
    assert_eq!(client.pending_proposals_count(), 1);

    // Advance time past timelock.
    s.env
        .ledger()
        .with_mut(|l| l.timestamp += PAY_DEFAULT_TIMELOCK_SECS + 1);

    let usdc_client = TokenClient::new(&s.env, &s.usdc);
    let bal_before = usdc_client.balance(&agency);
    client.execute_pay_default(&id);
    let bal_after = usdc_client.balance(&agency);
    assert_eq!(bal_after - bal_before, amount);
    assert_eq!(client.pending_proposals_count(), 0);
}

#[test]
#[should_panic]
fn execute_before_timelock_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);

    let agency = Address::generate(&s.env);
    let guarantee_hash = BytesN::from_array(&s.env, &[7u8; 32]);
    let item = PayDefaultItem {
        asset: s.usdc.clone(),
        amount: 50_000_000_000,
        destination: agency,
        guarantee_contract_hash: guarantee_hash,
        covered_month: 202609,
    };
    let mut items = Vec::new(&s.env);
    items.push_back(item);

    let id = client.propose_pay_default(&items);
    client.execute_pay_default(&id); // no time advance — should panic
}

#[test]
#[should_panic]
fn pay_default_item_above_max_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let agency = Address::generate(&s.env);
    let guarantee_hash = BytesN::from_array(&s.env, &[7u8; 32]);
    let item = PayDefaultItem {
        asset: s.usdc.clone(),
        amount: PAY_DEFAULT_MAX_ITEM_VALUE + 1,
        destination: agency,
        guarantee_contract_hash: guarantee_hash,
        covered_month: 202609,
    };
    let mut items = Vec::new(&s.env);
    items.push_back(item);
    client.propose_pay_default(&items);
}

#[test]
fn operator_outbound_to_whitelisted_destination() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);

    let op_tx_hash = BytesN::from_array(&s.env, &[2u8; 32]);
    client.operator_outbound(
        &OutboundKind::YieldAssetSubscription,
        &s.usdc,
        &100_000_000_000, // 10k USDC
        &s.op_dest,
        &op_tx_hash,
    );

    // PendingSwap is recorded.
    assert_eq!(client.pending_swap_value(&s.usdc), 100_000_000_000);
}

#[test]
#[should_panic]
fn operator_outbound_to_non_whitelisted_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let attacker = Address::generate(&s.env);
    let op_tx_hash = BytesN::from_array(&s.env, &[3u8; 32]);

    client.operator_outbound(
        &OutboundKind::YieldAssetSubscription,
        &s.usdc,
        &100_000_000_000,
        &attacker,
        &op_tx_hash,
    );
}

#[test]
fn operator_outbound_then_record_swap_in_clears_pending() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    let tesouro_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.tesouro);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);
    tesouro_token.mint(&s.vault_id, &0); // ensure registered

    let op_tx_hash = BytesN::from_array(&s.env, &[4u8; 32]);
    client.operator_outbound(
        &OutboundKind::YieldAssetSubscription,
        &s.usdc,
        &100_000_000_000,
        &s.op_dest,
        &op_tx_hash,
    );
    assert_eq!(client.pending_swap_value(&s.usdc), 100_000_000_000);

    // Simulate TESOURO arriving at vault.
    tesouro_token.mint(&s.vault_id, &99_800_000_000); // slight slippage

    let in_tx_hash = BytesN::from_array(&s.env, &[5u8; 32]);
    client.record_swap_in(&s.tesouro, &99_800_000_000, &op_tx_hash, &in_tx_hash);

    assert_eq!(client.pending_swap_value(&s.usdc), 0);
    assert_eq!(client.balance(&s.tesouro), 99_800_000_000);
}

#[test]
fn set_paused_blocks_pay_default() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    client.set_paused(&true);
    assert!(client.paused());

    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);
    let agency = Address::generate(&s.env);
    let guarantee_hash = BytesN::from_array(&s.env, &[8u8; 32]);
    let item = PayDefaultItem {
        asset: s.usdc.clone(),
        amount: 50_000_000_000,
        destination: agency,
        guarantee_contract_hash: guarantee_hash,
        covered_month: 202609,
    };
    let mut items = Vec::new(&s.env);
    items.push_back(item);

    let result = client.try_propose_pay_default(&items);
    assert!(result.is_err());
}

#[test]
fn two_step_admin_handover() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let new_admin = Address::generate(&s.env);

    client.propose_admin(&new_admin);
    assert_eq!(client.pending_admin(), Some(new_admin.clone()));

    client.accept_admin();
    assert_eq!(client.admin(), new_admin);
    assert_eq!(client.pending_admin(), None);
}

// ── finding #1: timelock floor (MIN_TIMELOCK_SECS = 3600) ───────────────────

#[test]
#[should_panic]
fn initialize_rejects_timelock_below_min() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    let other_admin = Address::generate(&env);
    let usdc = env
        .register_stellar_asset_contract_v2(other_admin)
        .address();
    let vault_id = env.register(ReserveVault, ());

    let mut approved = Vec::new(&env);
    approved.push_back(usdc.clone());

    // Try 60 seconds — below the 1-hour MIN_TIMELOCK_SECS floor.
    ReserveVaultClient::new(&env, &vault_id).initialize(
        &admin,
        &operator,
        &approved,
        &usdc,
        &Vec::new(&env),
        &PAY_DEFAULT_MAX_ITEM_VALUE,
        &60u64, // ← below MIN_TIMELOCK_SECS
        &MAX_ITEMS_PER_BATCH,
        &MAX_PENDING_PROPOSALS,
        &MAX_RATE_STALENESS_SECS,
    );
}

#[test]
#[should_panic]
fn set_pay_default_timelock_rejects_below_min() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    // Compromised-admin attack vector: try dropping to 60s.
    client.set_pay_default_timelock_secs(&60u64);
}

#[test]
fn set_pay_default_timelock_accepts_at_min() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    client.set_pay_default_timelock_secs(&3600u64); // exactly at min
    assert_eq!(client.pay_default_timelock_secs(), 3600);
}

// ── finding #4: set_denomination_asset rejects with pending proposals ──────

#[test]
#[should_panic]
fn set_denomination_rejects_when_proposals_pending() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);

    // Fund vault and queue a proposal.
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);
    let agency = Address::generate(&s.env);
    let guarantee_hash = BytesN::from_array(&s.env, &[42u8; 32]);
    let item = PayDefaultItem {
        asset: s.usdc.clone(),
        amount: 50_000_000_000,
        destination: agency,
        guarantee_contract_hash: guarantee_hash,
        covered_month: 202609,
    };
    let mut items = Vec::new(&s.env);
    items.push_back(item);
    client.propose_pay_default(&items);
    assert_eq!(client.pending_proposals_count(), 1);

    // Try to switch denomination while a proposal is pending — should panic.
    client.set_denomination_asset(&s.tesouro);
}

#[test]
fn set_denomination_succeeds_after_cancelling_pending() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);

    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);
    let agency = Address::generate(&s.env);
    let guarantee_hash = BytesN::from_array(&s.env, &[43u8; 32]);
    let item = PayDefaultItem {
        asset: s.usdc.clone(),
        amount: 50_000_000_000,
        destination: agency,
        guarantee_contract_hash: guarantee_hash,
        covered_month: 202609,
    };
    let mut items = Vec::new(&s.env);
    items.push_back(item);
    let id = client.propose_pay_default(&items);

    // Cancel, then denomination change must succeed.
    client.cancel_pay_default(&id);
    assert_eq!(client.pending_proposals_count(), 0);
    client.set_denomination_asset(&s.tesouro);
    assert_eq!(client.denomination_asset(), s.tesouro);
}
