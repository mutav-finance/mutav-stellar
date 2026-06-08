#![no_std]

//! Stage-1 reserve vault — `mutav-reserve-vault`.
//!
//! Stub. Audited surface for the on-chain Stage-1 reserve described in
//! `mutav-whitepaper.en.md` §5.4: holds Etherfuse TESOURO + a USDC liquidity
//! sleeve, exposes `total_assets()` for publishable adequacy checks, and
//! gates `cover_default` on the admin (cold) key.
//!
//! Design notes:
//! - No investor-deposit / quota-issuance surface. Stage-1 capitalization is
//!   the pre-seed raise, not subscriptions. Investor flows are Stage 2
//!   (`../../stage2/fund/`).
//! - No yield mechanic on any token this contract may interact with at the
//!   Stage-1 layer (§5.2: yield re-classifies as *valor mobiliário* under
//!   CVM Parecer 40/2022).
//! - Authority split mirrors §5.5: `admin` (cold, governance), `operator`
//!   (hot, ramp + accounting), `classic_wallet` (ramp hand-off — §11b open).
//!
//! Open items before first audit pass:
//! - `total_assets()` semantics: must read underlying TESOURO + USDC-sleeve
//!   balances, not a tracked accounting integer (§5.4.3).
//! - `cover_default` per-period cap + timelock (§11f).
//! - Weekly snapshot event for the §5.4.3 publication discipline.

use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct ReserveVault;

#[contractimpl]
impl ReserveVault {
    /// Placeholder until the §5.4 design lands. Returns 0.
    pub fn total_assets(_e: Env) -> i128 {
        0
    }
}
