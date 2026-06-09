#![no_std]
#![allow(deprecated)] // events().publish() is deprecated in favour of #[contractevent]; migrate later
#![allow(clippy::too_many_arguments)]

//! Stage-1 reserve vault — `mutav-reserve-vault`.
//!
//! Custodial holder of Mutav's pooled reserve backing *fiança onerosa*
//! guarantees per whitepaper §5.4. Single Soroban contract; no shares, no
//! NAV math, no SEP-56 vault. See
//! `docs/specs/2026-06-08-stage1-reserve-vault-design.md` for the design.
//!
//! Authority model:
//! - admin: governance + drain authority (pay_default). At Phase 1 deploy,
//!   this is an OZ Smart Account contract address (3-of-5 passkey multisig).
//!   For dev/testnet it can be any Stellar keypair.
//! - operator: medium-risk financial ops (capital records, swap outbounds,
//!   snapshots). Outbound destinations constrained on-chain to an admin-
//!   managed allowlist.
//!
//! Per-asset payment caps: each approved asset carries its own per-item
//! `pay_default` ceiling, set by admin at allowlist add. Valuation math
//! (rates between assets, denomination totals, K ratio) lives off-chain
//! in the Convex transparency portal — the vault exposes raw chain truth.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short,
    token::TokenClient, Address, BytesN, Env, Vec,
};

// ── constants ────────────────────────────────────────────────────────────────

// ~30 days in ledgers (assuming 5s/ledger).
const PERSISTENT_TTL: u32 = 518_400;
// ~7 days in ledgers — bounded replay-guard window.
const TEMPORARY_TTL: u32 = 50_400;

const MAX_APPROVED_ASSETS: u32 = 8;
const MAX_ALLOWED_DESTINATIONS: u32 = 16;
const MAX_ITEMS_PER_BATCH_CEILING: u32 = 200;
const MAX_PENDING_PROPOSALS_CEILING: u32 = 1000;

// Minimum pay_default timelock — defense against compromised admin lowering
// the timelock to seconds before draining. 1h gives ops a detection window
// while still allowing legitimate downward tuning during normal operations.
const MIN_TIMELOCK_SECS: u64 = 3600;

// ── error codes ──────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Paused = 3,
    BoundCheck = 5,
    AssetNotApproved = 6,
    AssetAlreadyApproved = 7,
    AssetBalanceNonzero = 8,
    DestinationNotAllowed = 10,
    DestinationAlreadyAllowed = 11,
    AmountMustBePositive = 12,
    ItemValueExceedsMax = 13,
    BatchEmpty = 15,
    BatchTooLarge = 16,
    PendingQueueFull = 17,
    ProposalNotFound = 18,
    TimelockNotExpired = 19,
    ReplayDetected = 20,
    SwapNotPending = 21,
    NoPendingAdmin = 22,
    InvalidValue = 24,
    AllowlistFull = 25,
    TimelockBelowMinimum = 27,
}

// ── storage keys ─────────────────────────────────────────────────────────────

#[contracttype]
enum DataKey {
    // governance
    Admin,
    Operator,
    PendingAdmin,
    Paused,

    // asset allowlist
    ApprovedAssets,
    // per-asset pay_default per-item ceiling (in native asset stroops)
    PayDefaultMaxItemValue(Address),

    // destination allowlist
    AllowedOutboundDestinations,

    // pay_default config
    PayDefaultTimelockSecs,
    MaxItemsPerBatch,
    MaxPendingProposals,

    // proposal queue
    NextProposalId,
    PendingProposalsCount,

    // persistent
    PendingSwap(BytesN<32>),
    PendingPayDefault(u64),
    PendingOutboundTotal(Address),

    // temporary
    SeenTxHash(BytesN<32>),
}

// ── data types ───────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Copy)]
pub enum OutboundKind {
    YieldAssetSubscription,
    YieldAssetRedemption,
}

#[contracttype]
#[derive(Clone)]
pub struct PendingSwapRecord {
    pub asset_out: Address,
    pub amount_out: i128,
    pub initiated_at: u64,
    pub kind: OutboundKind,
}

#[contracttype]
#[derive(Clone)]
pub struct PayDefaultItem {
    pub asset: Address,
    pub amount: i128,
    pub destination: Address,
    pub guarantee_contract_hash: BytesN<32>,
    pub covered_month: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct PayDefaultProposalRecord {
    pub items: Vec<PayDefaultItem>,
    pub propose_ts: u64,
    pub executable_after_ts: u64,
}

// ── storage helpers ──────────────────────────────────────────────────────────

fn get_admin(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, Error::NotInitialized))
}

fn get_operator(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&DataKey::Operator)
        .unwrap_or_else(|| panic_with_error!(e, Error::NotInitialized))
}

fn require_admin(e: &Env) {
    get_admin(e).require_auth();
}

fn require_operator(e: &Env) {
    get_operator(e).require_auth();
}

fn require_not_paused(e: &Env) {
    if e.storage()
        .instance()
        .get::<_, bool>(&DataKey::Paused)
        .unwrap_or(false)
    {
        panic_with_error!(e, Error::Paused);
    }
}

fn get_approved_assets(e: &Env) -> Vec<Address> {
    e.storage()
        .instance()
        .get(&DataKey::ApprovedAssets)
        .unwrap_or_else(|| Vec::new(e))
}

fn is_asset_approved(e: &Env, asset: &Address) -> bool {
    let assets = get_approved_assets(e);
    for i in 0..assets.len() {
        if &assets.get_unchecked(i) == asset {
            return true;
        }
    }
    false
}

fn get_allowed_destinations(e: &Env) -> Vec<Address> {
    e.storage()
        .instance()
        .get(&DataKey::AllowedOutboundDestinations)
        .unwrap_or_else(|| Vec::new(e))
}

fn is_destination_allowed(e: &Env, addr: &Address) -> bool {
    let dests = get_allowed_destinations(e);
    for i in 0..dests.len() {
        if &dests.get_unchecked(i) == addr {
            return true;
        }
    }
    false
}

fn get_pay_default_max_for(e: &Env, asset: &Address) -> i128 {
    e.storage()
        .instance()
        .get(&DataKey::PayDefaultMaxItemValue(asset.clone()))
        .unwrap_or_else(|| panic_with_error!(e, Error::AssetNotApproved))
}

fn get_pay_default_timelock(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get(&DataKey::PayDefaultTimelockSecs)
        .unwrap_or_else(|| panic_with_error!(e, Error::NotInitialized))
}

fn get_max_items_per_batch(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::MaxItemsPerBatch)
        .unwrap_or_else(|| panic_with_error!(e, Error::NotInitialized))
}

fn get_max_pending_proposals(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::MaxPendingProposals)
        .unwrap_or_else(|| panic_with_error!(e, Error::NotInitialized))
}

fn get_next_proposal_id(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get(&DataKey::NextProposalId)
        .unwrap_or(0)
}

fn get_pending_proposals_count(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::PendingProposalsCount)
        .unwrap_or(0)
}

fn check_replay(e: &Env, hash: &BytesN<32>) {
    let key = DataKey::SeenTxHash(hash.clone());
    if e.storage().temporary().has(&key) {
        panic_with_error!(e, Error::ReplayDetected);
    }
    e.storage().temporary().set(&key, &true);
    e.storage()
        .temporary()
        .extend_ttl(&key, TEMPORARY_TTL, TEMPORARY_TTL);
}

fn extend_persistent_ttl<K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(e: &Env, key: &K) {
    e.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_TTL, PERSISTENT_TTL);
}

// ── contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct ReserveVault;

#[contractimpl]
impl ReserveVault {
    /// One-shot init. Sets admin, operator, allowed destinations, and the
    /// pay_default config bounds. Approved assets and their per-asset caps
    /// are added separately via `add_approved_asset`.
    pub fn initialize(
        e: Env,
        admin: Address,
        operator: Address,
        initial_allowed_destinations: Vec<Address>,
        pay_default_timelock_secs: u64,
        max_items_per_batch: u32,
        max_pending_proposals: u32,
    ) {
        if e.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&e, Error::AlreadyInitialized);
        }
        if max_items_per_batch == 0
            || max_items_per_batch > MAX_ITEMS_PER_BATCH_CEILING
            || max_pending_proposals == 0
            || max_pending_proposals > MAX_PENDING_PROPOSALS_CEILING
            || initial_allowed_destinations.len() > MAX_ALLOWED_DESTINATIONS
        {
            panic_with_error!(&e, Error::BoundCheck);
        }
        if pay_default_timelock_secs < MIN_TIMELOCK_SECS {
            panic_with_error!(&e, Error::TimelockBelowMinimum);
        }

        let s = e.storage().instance();
        s.set(&DataKey::Admin, &admin);
        s.set(&DataKey::Operator, &operator);
        s.set(&DataKey::Paused, &false);
        s.set(&DataKey::ApprovedAssets, &Vec::<Address>::new(&e));
        s.set(
            &DataKey::AllowedOutboundDestinations,
            &initial_allowed_destinations,
        );
        s.set(&DataKey::PayDefaultTimelockSecs, &pay_default_timelock_secs);
        s.set(&DataKey::MaxItemsPerBatch, &max_items_per_batch);
        s.set(&DataKey::MaxPendingProposals, &max_pending_proposals);
        s.set(&DataKey::NextProposalId, &0u64);
        s.set(&DataKey::PendingProposalsCount, &0u32);
    }

    // ── admin: governance ────────────────────────────────────────────────────

    pub fn set_operator(e: Env, new_operator: Address) {
        require_admin(&e);
        e.storage()
            .instance()
            .set(&DataKey::Operator, &new_operator);
        e.events().publish((symbol_short!("set_op"),), new_operator);
    }

    pub fn set_paused(e: Env, paused: bool) {
        require_admin(&e);
        e.storage().instance().set(&DataKey::Paused, &paused);
        e.events().publish((symbol_short!("set_paus"),), paused);
    }

    pub fn propose_admin(e: Env, new_admin: Address) {
        require_admin(&e);
        e.storage()
            .instance()
            .set(&DataKey::PendingAdmin, &new_admin);
        e.events().publish((symbol_short!("prop_adm"),), new_admin);
    }

    pub fn accept_admin(e: Env) {
        let pending: Address = e
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .unwrap_or_else(|| panic_with_error!(&e, Error::NoPendingAdmin));
        pending.require_auth();
        e.storage().instance().set(&DataKey::Admin, &pending);
        e.storage().instance().remove(&DataKey::PendingAdmin);
        e.events().publish((symbol_short!("acc_adm"),), pending);
    }

    // ── admin: asset allowlist with per-asset caps ──────────────────────────

    /// Approve a new asset with its per-item pay_default ceiling (native
    /// stroops). The cap is enforced at every pay_default item that targets
    /// this asset.
    pub fn add_approved_asset(e: Env, asset: Address, max_item_value: i128) {
        require_admin(&e);
        if max_item_value <= 0 {
            panic_with_error!(&e, Error::InvalidValue);
        }
        let mut assets = get_approved_assets(&e);
        if assets.len() >= MAX_APPROVED_ASSETS {
            panic_with_error!(&e, Error::AllowlistFull);
        }
        if is_asset_approved(&e, &asset) {
            panic_with_error!(&e, Error::AssetAlreadyApproved);
        }
        assets.push_back(asset.clone());
        e.storage()
            .instance()
            .set(&DataKey::ApprovedAssets, &assets);
        e.storage().instance().set(
            &DataKey::PayDefaultMaxItemValue(asset.clone()),
            &max_item_value,
        );
        e.events()
            .publish((symbol_short!("asset_add"),), (asset, max_item_value));
    }

    pub fn remove_approved_asset(e: Env, asset: Address) {
        require_admin(&e);
        let token = TokenClient::new(&e, &asset);
        if token.balance(&e.current_contract_address()) > 0 {
            panic_with_error!(&e, Error::AssetBalanceNonzero);
        }
        let mut assets = get_approved_assets(&e);
        let mut found_at: Option<u32> = None;
        for i in 0..assets.len() {
            if assets.get_unchecked(i) == asset {
                found_at = Some(i);
                break;
            }
        }
        match found_at {
            Some(idx) => {
                assets.remove(idx);
                e.storage()
                    .instance()
                    .set(&DataKey::ApprovedAssets, &assets);
                e.storage()
                    .instance()
                    .remove(&DataKey::PayDefaultMaxItemValue(asset.clone()));
                e.events().publish((symbol_short!("asset_rm"),), asset);
            }
            None => panic_with_error!(&e, Error::AssetNotApproved),
        }
    }

    /// Update the per-asset pay_default per-item ceiling.
    pub fn set_pay_default_max_item_value(e: Env, asset: Address, value: i128) {
        require_admin(&e);
        if value <= 0 {
            panic_with_error!(&e, Error::InvalidValue);
        }
        if !is_asset_approved(&e, &asset) {
            panic_with_error!(&e, Error::AssetNotApproved);
        }
        e.storage()
            .instance()
            .set(&DataKey::PayDefaultMaxItemValue(asset.clone()), &value);
        e.events()
            .publish((symbol_short!("set_pmax"),), (asset, value));
    }

    // ── admin: destination allowlist ────────────────────────────────────────

    pub fn add_allowed_destination(e: Env, addr: Address) {
        require_admin(&e);
        let mut dests = get_allowed_destinations(&e);
        if dests.len() >= MAX_ALLOWED_DESTINATIONS {
            panic_with_error!(&e, Error::AllowlistFull);
        }
        if is_destination_allowed(&e, &addr) {
            panic_with_error!(&e, Error::DestinationAlreadyAllowed);
        }
        dests.push_back(addr.clone());
        e.storage()
            .instance()
            .set(&DataKey::AllowedOutboundDestinations, &dests);
        e.events().publish((symbol_short!("dest_add"),), addr);
    }

    pub fn remove_allowed_destination(e: Env, addr: Address) {
        require_admin(&e);
        let mut dests = get_allowed_destinations(&e);
        let mut found_at: Option<u32> = None;
        for i in 0..dests.len() {
            if dests.get_unchecked(i) == addr {
                found_at = Some(i);
                break;
            }
        }
        match found_at {
            Some(idx) => {
                dests.remove(idx);
                e.storage()
                    .instance()
                    .set(&DataKey::AllowedOutboundDestinations, &dests);
                e.events().publish((symbol_short!("dest_rm"),), addr);
            }
            None => panic_with_error!(&e, Error::DestinationNotAllowed),
        }
    }

    // ── admin: pay_default config ───────────────────────────────────────────

    pub fn set_pay_default_timelock_secs(e: Env, secs: u64) {
        require_admin(&e);
        if secs < MIN_TIMELOCK_SECS {
            panic_with_error!(&e, Error::TimelockBelowMinimum);
        }
        e.storage()
            .instance()
            .set(&DataKey::PayDefaultTimelockSecs, &secs);
        e.events().publish((symbol_short!("set_ptlk"),), secs);
    }

    pub fn set_max_items_per_batch(e: Env, value: u32) {
        require_admin(&e);
        if value == 0 || value > MAX_ITEMS_PER_BATCH_CEILING {
            panic_with_error!(&e, Error::InvalidValue);
        }
        e.storage()
            .instance()
            .set(&DataKey::MaxItemsPerBatch, &value);
        e.events().publish((symbol_short!("set_mib"),), value);
    }

    pub fn set_max_pending_proposals(e: Env, value: u32) {
        require_admin(&e);
        if value == 0 || value > MAX_PENDING_PROPOSALS_CEILING {
            panic_with_error!(&e, Error::InvalidValue);
        }
        e.storage()
            .instance()
            .set(&DataKey::MaxPendingProposals, &value);
        e.events().publish((symbol_short!("set_mpp"),), value);
    }

    // ── admin: pay_default lifecycle ────────────────────────────────────────

    pub fn propose_pay_default(e: Env, items: Vec<PayDefaultItem>) -> u64 {
        require_admin(&e);
        require_not_paused(&e);
        let item_count = items.len();
        if item_count == 0 {
            panic_with_error!(&e, Error::BatchEmpty);
        }
        if item_count > get_max_items_per_batch(&e) {
            panic_with_error!(&e, Error::BatchTooLarge);
        }
        if get_pending_proposals_count(&e) >= get_max_pending_proposals(&e) {
            panic_with_error!(&e, Error::PendingQueueFull);
        }

        for i in 0..item_count {
            let it = items.get_unchecked(i);
            if it.amount <= 0 {
                panic_with_error!(&e, Error::AmountMustBePositive);
            }
            if !is_asset_approved(&e, &it.asset) {
                panic_with_error!(&e, Error::AssetNotApproved);
            }
            let asset_max = get_pay_default_max_for(&e, &it.asset);
            if it.amount > asset_max {
                panic_with_error!(&e, Error::ItemValueExceedsMax);
            }
        }

        let id = get_next_proposal_id(&e);
        let propose_ts = e.ledger().timestamp();
        let executable_after_ts = propose_ts + get_pay_default_timelock(&e);
        let record = PayDefaultProposalRecord {
            items: items.clone(),
            propose_ts,
            executable_after_ts,
        };

        let key = DataKey::PendingPayDefault(id);
        e.storage().persistent().set(&key, &record);
        extend_persistent_ttl(&e, &key);

        e.storage()
            .instance()
            .set(&DataKey::NextProposalId, &(id + 1));
        let new_count = get_pending_proposals_count(&e) + 1;
        e.storage()
            .instance()
            .set(&DataKey::PendingProposalsCount, &new_count);

        e.events().publish(
            (symbol_short!("pay_prop"), id),
            (item_count, executable_after_ts, items),
        );
        id
    }

    pub fn execute_pay_default(e: Env, proposal_id: u64) {
        require_admin(&e);
        require_not_paused(&e);

        let key = DataKey::PendingPayDefault(proposal_id);
        let record: PayDefaultProposalRecord = e
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&e, Error::ProposalNotFound));
        let now = e.ledger().timestamp();
        if now < record.executable_after_ts {
            panic_with_error!(&e, Error::TimelockNotExpired);
        }

        let contract_addr = e.current_contract_address();

        for i in 0..record.items.len() {
            let it = record.items.get_unchecked(i);
            let token = TokenClient::new(&e, &it.asset);
            token.transfer(&contract_addr, &it.destination, &it.amount);
        }

        e.storage().persistent().remove(&key);
        let new_count = get_pending_proposals_count(&e).saturating_sub(1);
        e.storage()
            .instance()
            .set(&DataKey::PendingProposalsCount, &new_count);

        e.events().publish(
            (symbol_short!("pay_exec"), proposal_id),
            (record.items.len(), record.items),
        );
    }

    pub fn cancel_pay_default(e: Env, proposal_id: u64) {
        require_admin(&e);
        let key = DataKey::PendingPayDefault(proposal_id);
        if !e.storage().persistent().has(&key) {
            panic_with_error!(&e, Error::ProposalNotFound);
        }
        e.storage().persistent().remove(&key);
        let new_count = get_pending_proposals_count(&e).saturating_sub(1);
        e.storage()
            .instance()
            .set(&DataKey::PendingProposalsCount, &new_count);
        e.events()
            .publish((symbol_short!("pay_cncl"), proposal_id), ());
    }

    pub fn cancel_pending_swap(e: Env, op_tx_hash: BytesN<32>, justification_hash: BytesN<32>) {
        require_admin(&e);
        let key = DataKey::PendingSwap(op_tx_hash.clone());
        let record: PendingSwapRecord = e
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&e, Error::SwapNotPending));
        let pending_total = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::PendingOutboundTotal(record.asset_out.clone()))
            .unwrap_or(0);
        let new_total = pending_total - record.amount_out;
        e.storage().instance().set(
            &DataKey::PendingOutboundTotal(record.asset_out.clone()),
            &new_total,
        );
        e.storage().persistent().remove(&key);
        e.events()
            .publish((symbol_short!("swap_cncl"), op_tx_hash), justification_hash);
    }

    // ── operator: capital + swaps ───────────────────────────────────────────

    pub fn record_capital_receipt(
        e: Env,
        source: Address,
        asset: Address,
        amount: i128,
        src_tx_hash: BytesN<32>,
    ) {
        require_operator(&e);
        require_not_paused(&e);
        if amount <= 0 {
            panic_with_error!(&e, Error::AmountMustBePositive);
        }
        if !is_asset_approved(&e, &asset) {
            panic_with_error!(&e, Error::AssetNotApproved);
        }
        check_replay(&e, &src_tx_hash);
        e.events().publish(
            (symbol_short!("cap_in"), source),
            (asset, amount, src_tx_hash),
        );
    }

    pub fn operator_outbound(
        e: Env,
        kind: OutboundKind,
        asset: Address,
        amount: i128,
        destination: Address,
        op_tx_hash: BytesN<32>,
    ) {
        require_operator(&e);
        require_not_paused(&e);
        if amount <= 0 {
            panic_with_error!(&e, Error::AmountMustBePositive);
        }
        if !is_asset_approved(&e, &asset) {
            panic_with_error!(&e, Error::AssetNotApproved);
        }
        if !is_destination_allowed(&e, &destination) {
            panic_with_error!(&e, Error::DestinationNotAllowed);
        }
        check_replay(&e, &op_tx_hash);

        let token = TokenClient::new(&e, &asset);
        token.transfer(&e.current_contract_address(), &destination, &amount);

        let record = PendingSwapRecord {
            asset_out: asset.clone(),
            amount_out: amount,
            initiated_at: e.ledger().timestamp(),
            kind,
        };
        let key = DataKey::PendingSwap(op_tx_hash.clone());
        e.storage().persistent().set(&key, &record);
        extend_persistent_ttl(&e, &key);

        let pending_total = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::PendingOutboundTotal(asset.clone()))
            .unwrap_or(0);
        e.storage().instance().set(
            &DataKey::PendingOutboundTotal(asset.clone()),
            &(pending_total + amount),
        );

        e.events().publish(
            (symbol_short!("outbound"), op_tx_hash),
            (kind, asset, amount, destination),
        );
    }

    pub fn record_swap_in(
        e: Env,
        asset_in: Address,
        amount_in: i128,
        paired_outbound_ref: BytesN<32>,
        in_tx_hash: BytesN<32>,
    ) {
        require_operator(&e);
        require_not_paused(&e);
        if amount_in <= 0 {
            panic_with_error!(&e, Error::AmountMustBePositive);
        }
        if !is_asset_approved(&e, &asset_in) {
            panic_with_error!(&e, Error::AssetNotApproved);
        }
        check_replay(&e, &in_tx_hash);

        let key = DataKey::PendingSwap(paired_outbound_ref.clone());
        let record: PendingSwapRecord = e
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&e, Error::SwapNotPending));

        let pending_total = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::PendingOutboundTotal(record.asset_out.clone()))
            .unwrap_or(0);
        let new_total = pending_total - record.amount_out;
        e.storage().instance().set(
            &DataKey::PendingOutboundTotal(record.asset_out.clone()),
            &new_total,
        );
        e.storage().persistent().remove(&key);

        e.events().publish(
            (symbol_short!("swap_in"), in_tx_hash),
            (
                asset_in,
                amount_in,
                paired_outbound_ref,
                record.asset_out,
                record.amount_out,
            ),
        );
    }

    /// Publish a snapshot. Emits raw per-asset (balance, pending_outbound).
    /// The transparency portal applies valuation off-chain.
    pub fn publish_snapshot(e: Env) {
        require_operator(&e);
        let assets = get_approved_assets(&e);
        let mut per_asset: Vec<(Address, i128, i128)> = Vec::new(&e);
        for i in 0..assets.len() {
            let a = assets.get_unchecked(i);
            let token = TokenClient::new(&e, &a);
            let bal = token.balance(&e.current_contract_address());
            let pending = e
                .storage()
                .instance()
                .get::<_, i128>(&DataKey::PendingOutboundTotal(a.clone()))
                .unwrap_or(0);
            per_asset.push_back((a, bal, pending));
        }
        let published_at = e.ledger().timestamp();
        e.events()
            .publish((symbol_short!("snapshot"),), (per_asset, published_at));
    }

    // ── housekeeping ────────────────────────────────────────────────────────

    pub fn extend_ttl(e: Env) {
        require_operator(&e);
        e.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL, PERSISTENT_TTL);
    }

    // ── views ───────────────────────────────────────────────────────────────

    pub fn admin(e: Env) -> Address {
        get_admin(&e)
    }

    pub fn operator(e: Env) -> Address {
        get_operator(&e)
    }

    pub fn pending_admin(e: Env) -> Option<Address> {
        e.storage().instance().get(&DataKey::PendingAdmin)
    }

    pub fn paused(e: Env) -> bool {
        e.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    pub fn approved_assets(e: Env) -> Vec<Address> {
        get_approved_assets(&e)
    }

    pub fn is_approved_asset(e: Env, asset: Address) -> bool {
        is_asset_approved(&e, &asset)
    }

    pub fn pay_default_max_item_value(e: Env, asset: Address) -> i128 {
        get_pay_default_max_for(&e, &asset)
    }

    pub fn allowed_destinations(e: Env) -> Vec<Address> {
        get_allowed_destinations(&e)
    }

    pub fn is_destination_allowed(e: Env, addr: Address) -> bool {
        is_destination_allowed(&e, &addr)
    }

    pub fn balance(e: Env, asset: Address) -> i128 {
        let token = TokenClient::new(&e, &asset);
        token.balance(&e.current_contract_address())
    }

    pub fn pending_swap_value(e: Env, asset: Address) -> i128 {
        e.storage()
            .instance()
            .get::<_, i128>(&DataKey::PendingOutboundTotal(asset))
            .unwrap_or(0)
    }

    pub fn total_balance(e: Env, asset: Address) -> i128 {
        let token = TokenClient::new(&e, &asset);
        let bal = token.balance(&e.current_contract_address());
        let pending = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::PendingOutboundTotal(asset))
            .unwrap_or(0);
        bal + pending
    }

    pub fn pay_default_timelock_secs(e: Env) -> u64 {
        get_pay_default_timelock(&e)
    }

    pub fn max_items_per_batch(e: Env) -> u32 {
        get_max_items_per_batch(&e)
    }

    pub fn max_pending_proposals(e: Env) -> u32 {
        get_max_pending_proposals(&e)
    }

    pub fn pending_proposals_count(e: Env) -> u32 {
        get_pending_proposals_count(&e)
    }

    pub fn get_pay_default_proposal(e: Env, proposal_id: u64) -> Option<PayDefaultProposalRecord> {
        e.storage()
            .persistent()
            .get(&DataKey::PendingPayDefault(proposal_id))
    }
}

#[cfg(test)]
mod tests;
