#![no_std]

//! Stage-1 reserve vault — `mutav-reserve-vault`.
//!
//! A minimal custodial safe for Mutav's reserve backing *fiança onerosa*
//! guarantees per whitepaper §5.4. The contract is deliberately thin:
//!
//! - **Holds value** — SEP-41 token balances (admin-managed asset allowlist).
//! - **Authorizes one drain path** — `withdraw(asset, amount, destination, ref_hash)`,
//!   gated on `admin` authorization + asset/destination allowlist checks.
//! - **Nothing else** — no policy logic, no per-asset payment caps, no
//!   timelock, no batching, no rate table, no PendingSwap tracking, no
//!   yield distribution.
//!
//! All policy lives in the OZ Smart Account contract that holds the `admin`
//! authority. The Smart Account's Context Rules decide:
//!   - which signers can authorize a withdraw
//!   - what amount limits apply per asset
//!   - what per-period cumulative caps apply
//!   - what timelock applies to high-value withdraws
//!   - what differential thresholds apply to different operations
//!
//! The vault stays small enough to audit in a sitting; the Smart Account
//! configuration is audited separately. See
//! `docs/specs/2026-06-08-stage1-reserve-vault-design.md` for the design.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short,
    token::TokenClient, Address, BytesN, Env, Symbol, Vec,
};

// ── constants ────────────────────────────────────────────────────────────────

// Asset allowlist cap is small because every withdraw scans it linearly under
// admin auth budget. Raising it inflates the withdraw worst-case CPU cost.
const MAX_APPROVED_ASSETS: u32 = 8;
// Destination cap is larger because operator + treasury + Etherfuse + agency
// payout legs all live here; 64 covers projected agency count for the pilot.
const MAX_ALLOWED_DESTINATIONS: u32 = 64;

// ── error codes ──────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    Paused = 2,
    AssetNotApproved = 3,
    AssetAlreadyApproved = 4,
    AssetBalanceNonzero = 5,
    DestinationNotAllowed = 6,
    DestinationAlreadyAllowed = 7,
    AmountMustBePositive = 8,
    AllowlistFull = 9,
    NoPendingAdmin = 10,
    InvalidExpiry = 11,
    PendingAdminExpired = 12,
}

// ── storage keys ─────────────────────────────────────────────────────────────

#[contracttype]
enum DataKey {
    Admin,
    PendingAdmin,
    Paused,
    ApprovedAssets,
    AllowedDestinations,
}

/// Pending admin record stored in `temporary()` storage. The entry's TTL
/// matches `live_until_ledger` so it auto-GCs at the deadline; `accept_admin`
/// also re-checks `live_until_ledger` against the current ledger sequence to
/// defend against the entry being kept alive past its intended window.
#[contracttype]
#[derive(Clone)]
pub struct PendingAdmin {
    pub address: Address,
    pub live_until_ledger: u32,
}

// ── storage helpers ──────────────────────────────────────────────────────────

fn get_admin(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(e, Error::NotInitialized))
}

fn require_admin(e: &Env) {
    get_admin(e).require_auth();
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

// ── generic address-allowlist helpers ────────────────────────────────────────
//
// `ApprovedAssets` and `AllowedDestinations` are both `Vec<Address>` stored
// under a single `DataKey`. Collapsing the CRUD into one set of helpers keeps
// the two allowlists guaranteed-consistent (a fix applied to one cannot drift
// from the other) and shrinks audit surface.

fn allowlist_get(e: &Env, key: &DataKey) -> Vec<Address> {
    e.storage()
        .instance()
        .get(key)
        .unwrap_or_else(|| Vec::new(e))
}

fn allowlist_contains(e: &Env, key: &DataKey, addr: &Address) -> bool {
    let list = allowlist_get(e, key);
    for i in 0..list.len() {
        if &list.get_unchecked(i) == addr {
            return true;
        }
    }
    false
}

fn allowlist_add(e: &Env, key: &DataKey, cap: u32, addr: &Address, dup_err: Error) {
    let mut list = allowlist_get(e, key);
    if list.len() >= cap {
        panic_with_error!(e, Error::AllowlistFull);
    }
    if allowlist_contains(e, key, addr) {
        panic_with_error!(e, dup_err);
    }
    list.push_back(addr.clone());
    e.storage().instance().set(key, &list);
}

/// Swap-remove (O(1)): overwrite the target slot with the last element,
/// then pop. The allowlist is unordered so observable behavior is unchanged.
fn allowlist_remove(e: &Env, key: &DataKey, addr: &Address, missing_err: Error) {
    let mut list = allowlist_get(e, key);
    let mut found_at: Option<u32> = None;
    for i in 0..list.len() {
        if &list.get_unchecked(i) == addr {
            found_at = Some(i);
            break;
        }
    }
    match found_at {
        Some(idx) => {
            let last_idx = list.len() - 1;
            if idx != last_idx {
                let last = list.get_unchecked(last_idx);
                list.set(idx, last);
            }
            list.pop_back();
            e.storage().instance().set(key, &list);
        }
        None => panic_with_error!(e, missing_err),
    }
}

#[allow(deprecated)]
fn publish_event(e: &Env, topic: Symbol, value: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    e.events().publish((topic,), value);
}

// ── contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct ReserveVault;

#[contractimpl]
impl ReserveVault {
    /// Atomic init at deploy time — the constructor runs exactly once when
    /// the contract is created, removing the deploy → `initialize` race
    /// window. Per the soroban-sdk `__constructor` pattern (Protocol 22+).
    pub fn __constructor(e: Env, admin: Address) {
        e.storage().instance().set(&DataKey::Admin, &admin);
    }

    // ── admin: governance ───────────────────────────────────────────────────
    //
    // Note: `pause` is value-flow-only — it gates `withdraw`. Governance ops
    // (allowlist mutations, admin handover) remain available so a paused
    // vault can still be reconfigured during incident response.

    pub fn set_paused(e: Env, paused: bool) {
        require_admin(&e);
        e.storage().instance().set(&DataKey::Paused, &paused);
        publish_event(&e, symbol_short!("set_paus"), paused);
    }

    /// Two-step admin handover, step 1 — propose a new admin with an explicit
    /// ledger deadline. The pending entry is stored in `temporary()` storage
    /// so it auto-GCs at `live_until_ledger`; even if the admin forgets to
    /// cancel, the takeover surface evaporates on its own.
    ///
    /// Passing `live_until_ledger == 0` cancels any active proposal.
    ///
    /// Same-admin re-call overwrites the previous proposal (admin auth is
    /// required, so this is a deliberate replacement, not a silent overwrite).
    pub fn propose_admin(e: Env, new_admin: Address, live_until_ledger: u32) {
        require_admin(&e);
        if live_until_ledger == 0 {
            e.storage().temporary().remove(&DataKey::PendingAdmin);
            publish_event(&e, symbol_short!("prop_can"), ());
            return;
        }
        let current = e.ledger().sequence();
        if live_until_ledger < current || live_until_ledger > e.ledger().max_live_until_ledger() {
            panic_with_error!(&e, Error::InvalidExpiry);
        }
        let pending = PendingAdmin {
            address: new_admin.clone(),
            live_until_ledger,
        };
        e.storage()
            .temporary()
            .set(&DataKey::PendingAdmin, &pending);
        let live_for = live_until_ledger - current;
        e.storage()
            .temporary()
            .extend_ttl(&DataKey::PendingAdmin, live_for, live_for);
        publish_event(
            &e,
            symbol_short!("prop_adm"),
            (new_admin, live_until_ledger),
        );
    }

    /// Two-step admin handover, step 2 — pending admin signs to accept.
    /// Explicitly re-checks `live_until_ledger` against current ledger
    /// sequence, defending against a permissionless `extend_ttl` keeping the
    /// entry alive past its intended window.
    pub fn accept_admin(e: Env) {
        let pending: PendingAdmin = e
            .storage()
            .temporary()
            .get(&DataKey::PendingAdmin)
            .unwrap_or_else(|| panic_with_error!(&e, Error::NoPendingAdmin));
        if e.ledger().sequence() > pending.live_until_ledger {
            panic_with_error!(&e, Error::PendingAdminExpired);
        }
        pending.address.require_auth();
        e.storage()
            .instance()
            .set(&DataKey::Admin, &pending.address);
        e.storage().temporary().remove(&DataKey::PendingAdmin);
        publish_event(&e, symbol_short!("acc_adm"), pending.address);
    }

    // ── admin: asset allowlist ──────────────────────────────────────────────

    pub fn add_approved_asset(e: Env, asset: Address) {
        require_admin(&e);
        allowlist_add(
            &e,
            &DataKey::ApprovedAssets,
            MAX_APPROVED_ASSETS,
            &asset,
            Error::AssetAlreadyApproved,
        );
        publish_event(&e, symbol_short!("asset_add"), asset);
    }

    pub fn remove_approved_asset(e: Env, asset: Address) {
        require_admin(&e);
        // Hit-test the allowlist first so an unknown asset short-circuits
        // before the cross-contract balance() call (no wasted CPI on miss).
        if !allowlist_contains(&e, &DataKey::ApprovedAssets, &asset) {
            panic_with_error!(&e, Error::AssetNotApproved);
        }
        let token = TokenClient::new(&e, &asset);
        if token.balance(&e.current_contract_address()) > 0 {
            panic_with_error!(&e, Error::AssetBalanceNonzero);
        }
        allowlist_remove(
            &e,
            &DataKey::ApprovedAssets,
            &asset,
            Error::AssetNotApproved,
        );
        publish_event(&e, symbol_short!("asset_rm"), asset);
    }

    /// Escape hatch for asset deprecation: forcibly drop an asset from the
    /// allowlist even when balance > 0 (e.g., SEP-41 issuer was sanctioned,
    /// token is frozen and can't be withdrawn). Emits a distinct event with
    /// the balance-at-force as payload so the abandoned dust is explicit and
    /// auditable. Admin-only.
    pub fn force_remove_approved_asset(e: Env, asset: Address) {
        require_admin(&e);
        if !allowlist_contains(&e, &DataKey::ApprovedAssets, &asset) {
            panic_with_error!(&e, Error::AssetNotApproved);
        }
        let stranded_balance = TokenClient::new(&e, &asset).balance(&e.current_contract_address());
        allowlist_remove(
            &e,
            &DataKey::ApprovedAssets,
            &asset,
            Error::AssetNotApproved,
        );
        #[allow(deprecated)]
        e.events()
            .publish((symbol_short!("asset_frm"),), (asset, stranded_balance));
    }

    // ── admin: destination allowlist ────────────────────────────────────────

    pub fn add_allowed_destination(e: Env, addr: Address) {
        require_admin(&e);
        allowlist_add(
            &e,
            &DataKey::AllowedDestinations,
            MAX_ALLOWED_DESTINATIONS,
            &addr,
            Error::DestinationAlreadyAllowed,
        );
        publish_event(&e, symbol_short!("dest_add"), addr);
    }

    pub fn remove_allowed_destination(e: Env, addr: Address) {
        require_admin(&e);
        allowlist_remove(
            &e,
            &DataKey::AllowedDestinations,
            &addr,
            Error::DestinationNotAllowed,
        );
        publish_event(&e, symbol_short!("dest_rm"), addr);
    }

    // ── the one value-flow path ─────────────────────────────────────────────

    /// Withdraw `amount` of an approved `asset` to an allowed `destination`.
    /// Requires admin auth — the OZ Smart Account's Context Rules decide
    /// whether the specific call (which signers, what amount, what
    /// destination, what timing) is authorized.
    ///
    /// `ref_hash` is an opaque off-chain reference (e.g., guarantee + month,
    /// Etherfuse subscribe op ID, Pix endToEndId hash). The vault doesn't
    /// interpret it; the off-chain indexer correlates against Convex state.
    pub fn withdraw(
        e: Env,
        asset: Address,
        amount: i128,
        destination: Address,
        ref_hash: BytesN<32>,
    ) {
        require_admin(&e);
        require_not_paused(&e);
        if amount <= 0 {
            panic_with_error!(&e, Error::AmountMustBePositive);
        }
        if !allowlist_contains(&e, &DataKey::ApprovedAssets, &asset) {
            panic_with_error!(&e, Error::AssetNotApproved);
        }
        if !allowlist_contains(&e, &DataKey::AllowedDestinations, &destination) {
            panic_with_error!(&e, Error::DestinationNotAllowed);
        }
        let token = TokenClient::new(&e, &asset);
        token.transfer(&e.current_contract_address(), &destination, &amount);
        #[allow(deprecated)]
        e.events().publish(
            (symbol_short!("withdraw"), ref_hash),
            (asset, amount, destination),
        );
    }

    // ── views ───────────────────────────────────────────────────────────────

    pub fn admin(e: Env) -> Address {
        get_admin(&e)
    }

    pub fn pending_admin(e: Env) -> Option<PendingAdmin> {
        e.storage().temporary().get(&DataKey::PendingAdmin)
    }

    pub fn paused(e: Env) -> bool {
        e.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    pub fn approved_assets(e: Env) -> Vec<Address> {
        allowlist_get(&e, &DataKey::ApprovedAssets)
    }

    pub fn is_approved_asset(e: Env, asset: Address) -> bool {
        allowlist_contains(&e, &DataKey::ApprovedAssets, &asset)
    }

    pub fn allowed_destinations(e: Env) -> Vec<Address> {
        allowlist_get(&e, &DataKey::AllowedDestinations)
    }

    pub fn is_destination_allowed(e: Env, addr: Address) -> bool {
        allowlist_contains(&e, &DataKey::AllowedDestinations, &addr)
    }

    pub fn balance(e: Env, asset: Address) -> i128 {
        let token = TokenClient::new(&e, &asset);
        token.balance(&e.current_contract_address())
    }

    /// Permissionless TTL bump for instance storage. Anyone can call to keep
    /// the vault's instance entries (admin, allowlists, paused flag) from
    /// archival if the vault sits idle. ~30 days at 5s ledgers.
    pub fn extend_ttl(e: Env) {
        e.storage().instance().extend_ttl(518_400, 518_400);
    }
}

#[cfg(test)]
mod tests;
