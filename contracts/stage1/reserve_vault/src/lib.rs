#![no_std]
#![allow(deprecated)] // events().publish() is deprecated in favour of #[contractevent]; migrate later

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
    token::TokenClient, Address, BytesN, Env, Vec,
};

// ── constants ────────────────────────────────────────────────────────────────

const MAX_APPROVED_ASSETS: u32 = 8;
const MAX_ALLOWED_DESTINATIONS: u32 = 64; // larger than operator-only because agencies also live here

// ── error codes ──────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Paused = 3,
    AssetNotApproved = 4,
    AssetAlreadyApproved = 5,
    AssetBalanceNonzero = 6,
    DestinationNotAllowed = 7,
    DestinationAlreadyAllowed = 8,
    AmountMustBePositive = 9,
    AllowlistFull = 10,
    NoPendingAdmin = 11,
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
        .get(&DataKey::AllowedDestinations)
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

// ── contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct ReserveVault;

#[contractimpl]
impl ReserveVault {
    /// One-shot init. Sets admin (the OZ Smart Account address). All other
    /// state is empty at deploy; admin populates allowlists via the
    /// `add_*` entry points before the vault is usable for withdrawals.
    pub fn initialize(e: Env, admin: Address) {
        if e.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&e, Error::AlreadyInitialized);
        }
        let s = e.storage().instance();
        s.set(&DataKey::Admin, &admin);
        s.set(&DataKey::Paused, &false);
        s.set(&DataKey::ApprovedAssets, &Vec::<Address>::new(&e));
        s.set(&DataKey::AllowedDestinations, &Vec::<Address>::new(&e));
    }

    // ── admin: governance ───────────────────────────────────────────────────

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

    // ── admin: asset allowlist ──────────────────────────────────────────────

    pub fn add_approved_asset(e: Env, asset: Address) {
        require_admin(&e);
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
        e.events().publish((symbol_short!("asset_add"),), asset);
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
                e.events().publish((symbol_short!("asset_rm"),), asset);
            }
            None => panic_with_error!(&e, Error::AssetNotApproved),
        }
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
            .set(&DataKey::AllowedDestinations, &dests);
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
                    .set(&DataKey::AllowedDestinations, &dests);
                e.events().publish((symbol_short!("dest_rm"),), addr);
            }
            None => panic_with_error!(&e, Error::DestinationNotAllowed),
        }
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
        if !is_asset_approved(&e, &asset) {
            panic_with_error!(&e, Error::AssetNotApproved);
        }
        if !is_destination_allowed(&e, &destination) {
            panic_with_error!(&e, Error::DestinationNotAllowed);
        }
        let token = TokenClient::new(&e, &asset);
        token.transfer(&e.current_contract_address(), &destination, &amount);
        e.events().publish(
            (symbol_short!("withdraw"), ref_hash),
            (asset, amount, destination),
        );
    }

    // ── views ───────────────────────────────────────────────────────────────

    pub fn admin(e: Env) -> Address {
        get_admin(&e)
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

    /// Permissionless TTL bump for instance storage. Anyone can call to keep
    /// the vault's instance entries (admin, allowlists, paused flag) from
    /// archival if the vault sits idle. ~30 days at 5s ledgers.
    pub fn extend_ttl(e: Env) {
        e.storage().instance().extend_ttl(518_400, 518_400);
    }
}

#[cfg(test)]
mod tests;
