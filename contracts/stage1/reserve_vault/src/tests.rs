#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Env,
};

const USDC_MAX_ITEM: i128 = 150_000_000_000; // 15k USDC (7 decimals)
const TESOURO_MAX_ITEM: i128 = 150_000_000_000; // 15k TESOURO (7 decimals)
const PAY_DEFAULT_TIMELOCK_SECS: u64 = 86_400; // 24h
const MAX_ITEMS_PER_BATCH: u32 = 50;
const MAX_PENDING_PROPOSALS: u32 = 100;

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

    let mut dests = Vec::new(&env);
    dests.push_back(op_dest.clone());

    let client = ReserveVaultClient::new(&env, &vault_id);

    client.initialize(
        &admin,
        &operator,
        &dests,
        &PAY_DEFAULT_TIMELOCK_SECS,
        &MAX_ITEMS_PER_BATCH,
        &MAX_PENDING_PROPOSALS,
    );

    // Add the two test assets with their per-asset caps.
    client.add_approved_asset(&usdc, &USDC_MAX_ITEM);
    client.add_approved_asset(&tesouro, &TESOURO_MAX_ITEM);

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
    assert_eq!(client.approved_assets().len(), 2);
    assert_eq!(client.allowed_destinations().len(), 1);
    assert!(client.is_destination_allowed(&s.op_dest));
    assert_eq!(client.pay_default_max_item_value(&s.usdc), USDC_MAX_ITEM);
    assert_eq!(
        client.pay_default_max_item_value(&s.tesouro),
        TESOURO_MAX_ITEM
    );
}

#[test]
#[should_panic]
fn initialize_rejects_timelock_below_min() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    let vault_id = env.register(ReserveVault, ());

    ReserveVaultClient::new(&env, &vault_id).initialize(
        &admin,
        &operator,
        &Vec::new(&env),
        &60u64, // ← below MIN_TIMELOCK_SECS (3600)
        &MAX_ITEMS_PER_BATCH,
        &MAX_PENDING_PROPOSALS,
    );
}

#[test]
fn add_remove_approved_asset_with_cap() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let new_admin = Address::generate(&s.env);
    let other_asset = s
        .env
        .register_stellar_asset_contract_v2(new_admin)
        .address();
    let other_cap = 200_000_000_000i128;

    client.add_approved_asset(&other_asset, &other_cap);
    assert!(client.is_approved_asset(&other_asset));
    assert_eq!(client.pay_default_max_item_value(&other_asset), other_cap);
    assert_eq!(client.approved_assets().len(), 3);

    client.remove_approved_asset(&other_asset);
    assert!(!client.is_approved_asset(&other_asset));
    assert_eq!(client.approved_assets().len(), 2);
}

#[test]
#[should_panic]
fn add_approved_asset_rejects_zero_cap() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let new_admin = Address::generate(&s.env);
    let other_asset = s
        .env
        .register_stellar_asset_contract_v2(new_admin)
        .address();
    client.add_approved_asset(&other_asset, &0i128);
}

#[test]
fn set_pay_default_max_item_value_per_asset() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let new_cap = 250_000_000_000i128;
    client.set_pay_default_max_item_value(&s.usdc, &new_cap);
    assert_eq!(client.pay_default_max_item_value(&s.usdc), new_cap);
    // TESOURO cap unaffected
    assert_eq!(
        client.pay_default_max_item_value(&s.tesouro),
        TESOURO_MAX_ITEM
    );
}

#[test]
#[should_panic]
fn set_pay_default_max_for_unapproved_asset_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let random = Address::generate(&s.env);
    client.set_pay_default_max_item_value(&random, &100_000_000_000);
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
fn pay_default_full_lifecycle() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let usdc_token = soroban_sdk::token::StellarAssetClient::new(&s.env, &s.usdc);
    usdc_token.mint(&s.vault_id, &1_000_000_000_000);

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

    s.env
        .ledger()
        .with_mut(|l| l.timestamp += PAY_DEFAULT_TIMELOCK_SECS + 1);

    let token = TokenClient::new(&s.env, &s.usdc);
    let bal_before = token.balance(&agency);
    client.execute_pay_default(&id);
    let bal_after = token.balance(&agency);
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
    client.execute_pay_default(&id);
}

#[test]
#[should_panic]
fn pay_default_item_above_per_asset_max_panics() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    let agency = Address::generate(&s.env);
    let guarantee_hash = BytesN::from_array(&s.env, &[7u8; 32]);
    let item = PayDefaultItem {
        asset: s.usdc.clone(),
        amount: USDC_MAX_ITEM + 1, // exceeds USDC's per-asset cap
        destination: agency,
        guarantee_contract_hash: guarantee_hash,
        covered_month: 202609,
    };
    let mut items = Vec::new(&s.env);
    items.push_back(item);
    client.propose_pay_default(&items);
}

#[test]
fn pay_default_caps_are_independent_per_asset() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);

    // Tighten USDC cap to 5k; keep TESOURO at 15k.
    let new_usdc_cap = 50_000_000_000i128;
    client.set_pay_default_max_item_value(&s.usdc, &new_usdc_cap);
    assert_eq!(client.pay_default_max_item_value(&s.usdc), new_usdc_cap);
    assert_eq!(
        client.pay_default_max_item_value(&s.tesouro),
        TESOURO_MAX_ITEM
    );

    // A 6k USDC item should now fail (over the tightened cap)
    let agency = Address::generate(&s.env);
    let guarantee_hash = BytesN::from_array(&s.env, &[9u8; 32]);
    let usdc_item = PayDefaultItem {
        asset: s.usdc.clone(),
        amount: 60_000_000_000,
        destination: agency.clone(),
        guarantee_contract_hash: guarantee_hash.clone(),
        covered_month: 202609,
    };
    let mut items = Vec::new(&s.env);
    items.push_back(usdc_item);
    assert!(client.try_propose_pay_default(&items).is_err());

    // But a 14k TESOURO item should still pass (under the 15k TESOURO cap)
    let tesouro_item = PayDefaultItem {
        asset: s.tesouro.clone(),
        amount: 140_000_000_000,
        destination: agency,
        guarantee_contract_hash: guarantee_hash,
        covered_month: 202609,
    };
    let mut t_items = Vec::new(&s.env);
    t_items.push_back(tesouro_item);
    client.propose_pay_default(&t_items);
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
        &100_000_000_000,
        &s.op_dest,
        &op_tx_hash,
    );

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
    tesouro_token.mint(&s.vault_id, &0);

    let op_tx_hash = BytesN::from_array(&s.env, &[4u8; 32]);
    client.operator_outbound(
        &OutboundKind::YieldAssetSubscription,
        &s.usdc,
        &100_000_000_000,
        &s.op_dest,
        &op_tx_hash,
    );
    assert_eq!(client.pending_swap_value(&s.usdc), 100_000_000_000);

    tesouro_token.mint(&s.vault_id, &99_800_000_000);

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

#[test]
#[should_panic]
fn set_pay_default_timelock_rejects_below_min() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    client.set_pay_default_timelock_secs(&60u64);
}

#[test]
fn set_pay_default_timelock_accepts_at_min() {
    let s = setup();
    let client = ReserveVaultClient::new(&s.env, &s.vault_id);
    client.set_pay_default_timelock_secs(&3600u64);
    assert_eq!(client.pay_default_timelock_secs(), 3600);
}
