#![no_std]
#![allow(deprecated)] // events().publish() is deprecated in favour of #[contractevent]; migrate later

use soroban_sdk::{
    contract, contractimpl, contracttype,
    symbol_short,
    token::{self, TokenInterface},
    Address, Env, MuxedAddress, String,
};

// 30 days in seconds — minimum interval between management fee charges
const MIN_FEE_INTERVAL: u64 = 30 * 24 * 60 * 60;

// ── storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
enum DataKey {
    // fund accounting (instance storage — always loaded)
    Admin,
    ProtocolAddr,
    UsdcToken,
    Aum,
    TotalSupply,
    LastFeeTimestamp,
    // token accounting (persistent storage — per-address)
    Balance(Address),
    Allowance(AllowanceKey),
    // token metadata (instance storage)
    TokenMeta,
}

#[contracttype]
#[derive(Clone)]
struct AllowanceKey {
    from: Address,
    spender: Address,
}

#[contracttype]
#[derive(Clone)]
struct AllowanceValue {
    amount: i128,
    expiration_ledger: u32,
}

#[contracttype]
#[derive(Clone)]
struct TokenMeta {
    decimal: u32,
    name: String,
    symbol: String,
}

// ── storage helpers ───────────────────────────────────────────────────────────

fn get_admin(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Admin).expect("not initialized")
}

fn get_aum(e: &Env) -> i128 {
    e.storage().instance().get(&DataKey::Aum).unwrap_or(0)
}

fn set_aum(e: &Env, v: i128) {
    e.storage().instance().set(&DataKey::Aum, &v);
}

fn get_supply(e: &Env) -> i128 {
    e.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0)
}

fn set_supply(e: &Env, v: i128) {
    e.storage().instance().set(&DataKey::TotalSupply, &v);
}

fn get_protocol_addr(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::ProtocolAddr).expect("not initialized")
}

fn get_usdc_token(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::UsdcToken).expect("not initialized")
}

fn get_last_fee_ts(e: &Env) -> u64 {
    e.storage().instance().get(&DataKey::LastFeeTimestamp).unwrap_or(0)
}

fn set_last_fee_ts(e: &Env, v: u64) {
    e.storage().instance().set(&DataKey::LastFeeTimestamp, &v);
}

fn balance_of(e: &Env, addr: &Address) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::Balance(addr.clone()))
        .unwrap_or(0)
}

fn write_balance(e: &Env, addr: &Address, amount: i128) {
    let key = DataKey::Balance(addr.clone());
    e.storage().persistent().set(&key, &amount);
    e.storage().persistent().extend_ttl(&key, 518_400, 518_400);
}

fn get_allowance(e: &Env, from: &Address, spender: &Address) -> AllowanceValue {
    let key = DataKey::Allowance(AllowanceKey {
        from: from.clone(),
        spender: spender.clone(),
    });
    e.storage()
        .temporary()
        .get(&key)
        .unwrap_or(AllowanceValue {
            amount: 0,
            expiration_ledger: 0,
        })
}

fn write_allowance(
    e: &Env,
    from: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) {
    let key = DataKey::Allowance(AllowanceKey {
        from: from.clone(),
        spender: spender.clone(),
    });
    e.storage()
        .temporary()
        .set(&key, &AllowanceValue { amount, expiration_ledger });

    if amount > 0 {
        let current = e.ledger().sequence();
        assert!(expiration_ledger >= current, "expiration_ledger must be >= current ledger");
        let live_for = expiration_ledger - current;
        e.storage().temporary().extend_ttl(&key, live_for, live_for);
    }
}

fn require_admin(e: &Env) {
    get_admin(e).require_auth();
}

// ── NAV math ──────────────────────────────────────────────────────────────────
//
// All USDC amounts are in micro-USDC (7 decimal places, same as Stellar stroops).
// All MUTAV amounts are also 7 decimal places.
//
// NAV  = AUM / supply                         (USDC units per MUTAV unit)
// Mint = amount_usdc * supply / aum            (proportional; 1:1 when supply == 0)
// Burn = mutav_amount * aum / supply           (proportional redemption)
//
// Integer division truncates, which slightly benefits the fund on each operation.

fn calc_mint(amount_usdc: i128, aum: i128, supply: i128) -> i128 {
    if supply == 0 {
        amount_usdc // first deposit: 1 USDC = 1 MUTAV
    } else {
        amount_usdc * supply / aum
    }
}

fn calc_redeem(mutav_amount: i128, aum: i128, supply: i128) -> i128 {
    mutav_amount * aum / supply
}

// ── contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct Fund;

#[contractimpl]
impl Fund {
    /// Deploy and configure the fund. Called once.
    pub fn initialize(
        e: Env,
        admin: Address,
        protocol_addr: Address,
        usdc_token: Address,
    ) {
        if e.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage().instance().set(&DataKey::ProtocolAddr, &protocol_addr);
        e.storage().instance().set(&DataKey::UsdcToken, &usdc_token);
        e.storage().instance().set(
            &DataKey::TokenMeta,
            &TokenMeta {
                decimal: 7,
                name: String::from_str(&e, "MUTAV"),
                symbol: String::from_str(&e, "MUTAV"),
            },
        );
    }

    // ── investor operations ───────────────────────────────────────────────────

    /// Investor deposits USDC and receives MUTAV tokens at the current NAV.
    /// Investor must have approved this contract to spend `amount_usdc` of USDC.
    pub fn deposit_investor(e: Env, investor: Address, amount_usdc: i128) {
        investor.require_auth();
        assert!(amount_usdc > 0, "amount must be positive");

        // pull USDC from investor — requires investor auth on this sub-invocation
        token::Client::new(&e, &get_usdc_token(&e)).transfer(
            &investor,
            &e.current_contract_address(),
            &amount_usdc,
        );

        let aum = get_aum(&e);
        let supply = get_supply(&e);
        let tokens = calc_mint(amount_usdc, aum, supply);
        assert!(tokens > 0, "deposit too small relative to NAV");

        write_balance(&e, &investor, balance_of(&e, &investor) + tokens);
        set_supply(&e, supply + tokens);
        set_aum(&e, aum + amount_usdc);

        e.events().publish(
            (symbol_short!("deposit"), investor.clone()),
            (amount_usdc, tokens),
        );
    }

    /// Investor redeems MUTAV tokens for USDC at the current NAV.
    pub fn withdraw_investor(e: Env, investor: Address, mutav_amount: i128) {
        investor.require_auth();
        assert!(mutav_amount > 0, "amount must be positive");

        let aum = get_aum(&e);
        let supply = get_supply(&e);
        assert!(supply > 0, "no supply");

        let bal = balance_of(&e, &investor);
        assert!(bal >= mutav_amount, "insufficient balance");

        let usdc_out = calc_redeem(mutav_amount, aum, supply);
        assert!(usdc_out > 0, "amount too small relative to NAV");

        write_balance(&e, &investor, bal - mutav_amount);
        set_supply(&e, supply - mutav_amount);
        set_aum(&e, aum - usdc_out);

        token::Client::new(&e, &get_usdc_token(&e)).transfer(
            &e.current_contract_address(),
            &investor,
            &usdc_out,
        );

        e.events().publish(
            (symbol_short!("withdraw"), investor.clone()),
            (mutav_amount, usdc_out),
        );
    }

    // ── admin fund operations ─────────────────────────────────────────────────

    /// Record incoming tenant fees (80% of collected fee, already in the fund
    /// contract's USDC balance). Increases AUM → NAV increases for token holders.
    pub fn add_tenant_fee(e: Env, amount_usdc: i128) {
        require_admin(&e);
        assert!(amount_usdc > 0, "amount must be positive");
        set_aum(&e, get_aum(&e) + amount_usdc);
        e.events().publish((symbol_short!("fee_in"),), (amount_usdc,));
    }

    /// Record incoming yield from tokenized treasury (USDC, already in the fund
    /// contract's balance). Increases AUM → NAV increases for token holders.
    pub fn add_yield(e: Env, amount_usdc: i128) {
        require_admin(&e);
        assert!(amount_usdc > 0, "amount must be positive");
        set_aum(&e, get_aum(&e) + amount_usdc);
        e.events().publish((symbol_short!("yield_in"),), (amount_usdc,));
    }

    /// Charge the 1%/month management fee. Transfers 1% of AUM to the protocol
    /// wallet and decreases AUM (NAV decreases). Enforces a 30-day minimum
    /// interval to prevent double-charging.
    pub fn charge_mgmt_fee(e: Env) {
        require_admin(&e);

        let now = e.ledger().timestamp();
        let last = get_last_fee_ts(&e);
        assert!(
            now >= last + MIN_FEE_INTERVAL,
            "management fee already charged this period"
        );

        let aum = get_aum(&e);
        let fee = aum / 100; // 1%
        assert!(fee > 0, "AUM too small to charge fee");

        token::Client::new(&e, &get_usdc_token(&e)).transfer(
            &e.current_contract_address(),
            &get_protocol_addr(&e),
            &fee,
        );

        set_aum(&e, aum - fee);
        set_last_fee_ts(&e, now);

        e.events().publish((symbol_short!("mgmt_fee"),), (fee,));
    }

    /// Cover a default: USDC leaves the fund to pay the landlord. AUM decreases,
    /// no tokens burned → NAV decreases (loss absorbed by all token holders).
    pub fn cover_default(e: Env, amount_usdc: i128, destination: Address) {
        require_admin(&e);
        assert!(amount_usdc > 0, "amount must be positive");

        let aum = get_aum(&e);
        assert!(aum >= amount_usdc, "insufficient AUM");

        token::Client::new(&e, &get_usdc_token(&e)).transfer(
            &e.current_contract_address(),
            &destination,
            &amount_usdc,
        );

        set_aum(&e, aum - amount_usdc);

        e.events().publish(
            (symbol_short!("default"), destination.clone()),
            (amount_usdc,),
        );
    }

    // ── view ──────────────────────────────────────────────────────────────────

    /// Current NAV, scaled to 7 decimal places.
    /// Example: 10_500_000 means 1.05 USDC per MUTAV token.
    pub fn nav(e: Env) -> i128 {
        let supply = get_supply(&e);
        if supply == 0 {
            return 10_000_000; // 1.0 — initial NAV before any deposits
        }
        get_aum(&e) * 10_000_000 / supply
    }

    pub fn aum(e: Env) -> i128 {
        get_aum(&e)
    }

    pub fn total_supply(e: Env) -> i128 {
        get_supply(&e)
    }

    pub fn set_admin(e: Env, new_admin: Address) {
        require_admin(&e);
        e.storage().instance().set(&DataKey::Admin, &new_admin);
    }
}

// ── SEP-0041 token interface ──────────────────────────────────────────────────
//
// The MUTAV fund token IS the fund contract. Token holders can transfer their
// position freely. The proper redemption path is `withdraw_investor`, not `burn`.
// A raw `burn` destroys tokens without touching AUM (NAV increases for others).

#[contractimpl]
impl TokenInterface for Fund {
    fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        let a = get_allowance(&e, &from, &spender);
        if a.expiration_ledger < e.ledger().sequence() {
            return 0;
        }
        a.amount
    }

    fn approve(
        e: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        from.require_auth();
        assert!(amount >= 0, "amount must be non-negative");
        write_allowance(&e, &from, &spender, amount, expiration_ledger);
        e.events().publish(
            (symbol_short!("approve"), from.clone()),
            (spender, amount, expiration_ledger),
        );
    }

    fn balance(e: Env, id: Address) -> i128 {
        balance_of(&e, &id)
    }

    fn transfer(e: Env, from: Address, to: MuxedAddress, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "amount must be positive");
        // Strip the mux-ID: balances are always keyed by the base Address.
        let to_addr = to.address();
        let bal = balance_of(&e, &from);
        assert!(bal >= amount, "insufficient balance");
        write_balance(&e, &from, bal - amount);
        write_balance(&e, &to_addr, balance_of(&e, &to_addr) + amount);
        e.events().publish(
            (symbol_short!("transfer"), from.clone()),
            (to_addr, amount),
        );
    }

    fn transfer_from(
        e: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) {
        spender.require_auth();
        assert!(amount > 0, "amount must be positive");

        let a = get_allowance(&e, &from, &spender);
        assert!(a.expiration_ledger >= e.ledger().sequence(), "allowance expired");
        assert!(a.amount >= amount, "insufficient allowance");

        write_allowance(&e, &from, &spender, a.amount - amount, a.expiration_ledger);

        let bal = balance_of(&e, &from);
        assert!(bal >= amount, "insufficient balance");
        write_balance(&e, &from, bal - amount);
        write_balance(&e, &to, balance_of(&e, &to) + amount);

        e.events().publish(
            (symbol_short!("transfer"), from.clone()),
            (to, amount),
        );
    }

    fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "amount must be positive");
        let bal = balance_of(&e, &from);
        assert!(bal >= amount, "insufficient balance");
        write_balance(&e, &from, bal - amount);
        set_supply(&e, get_supply(&e) - amount);
        e.events().publish((symbol_short!("burn"), from.clone()), (amount,));
    }

    fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        assert!(amount > 0, "amount must be positive");

        let a = get_allowance(&e, &from, &spender);
        assert!(a.expiration_ledger >= e.ledger().sequence(), "allowance expired");
        assert!(a.amount >= amount, "insufficient allowance");

        write_allowance(&e, &from, &spender, a.amount - amount, a.expiration_ledger);

        let bal = balance_of(&e, &from);
        assert!(bal >= amount, "insufficient balance");
        write_balance(&e, &from, bal - amount);
        set_supply(&e, get_supply(&e) - amount);

        e.events().publish((symbol_short!("burn"), from.clone()), (amount,));
    }

    fn decimals(_e: Env) -> u32 {
        7
    }

    fn name(e: Env) -> String {
        let meta: TokenMeta = e
            .storage()
            .instance()
            .get(&DataKey::TokenMeta)
            .expect("not initialized");
        meta.name
    }

    fn symbol(e: Env) -> String {
        let meta: TokenMeta = e
            .storage()
            .instance()
            .get(&DataKey::TokenMeta)
            .expect("not initialized");
        meta.symbol
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger}, Env};

    struct Setup {
        env: Env,
        admin: Address,
        protocol: Address,
        fund_id: Address,
        usdc_id: Address,
    }

    fn setup() -> Setup {
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);
        let protocol = Address::generate(&e);

        let usdc_id = e.register_stellar_asset_contract_v2(admin.clone()).address();
        let fund_id = e.register(Fund, ());

        FundClient::new(&e, &fund_id).initialize(&admin, &protocol, &usdc_id);

        Setup { env: e, admin, protocol, fund_id, usdc_id }
    }

    fn usdc_mint(s: &Setup, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&s.env, &s.usdc_id).mint(to, &amount);
    }

    #[test]
    fn first_deposit_is_one_to_one() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000); // 10 USDC (7 decimals)
        fund.deposit_investor(&investor, &100_000_000);

        assert_eq!(fund.total_supply(), 100_000_000);
        assert_eq!(fund.aum(), 100_000_000);
        assert_eq!(fund.nav(), 10_000_000); // 1.0
        assert_eq!(fund.balance(&investor), 100_000_000);
    }

    #[test]
    fn second_deposit_is_proportional() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let alice = Address::generate(&s.env);
        let bob = Address::generate(&s.env);

        usdc_mint(&s, &alice, 100_000_000);
        fund.deposit_investor(&alice, &100_000_000);

        // Backend sent 100 USDC to fund address and calls add_tenant_fee to record it.
        usdc_mint(&s, &s.fund_id, 100_000_000);
        fund.add_tenant_fee(&100_000_000);
        assert_eq!(fund.nav(), 20_000_000); // NAV = 2.0

        // Bob deposits 100 USDC at NAV=2 → should receive 50 MUTAV
        usdc_mint(&s, &bob, 100_000_000);
        fund.deposit_investor(&bob, &100_000_000);

        assert_eq!(fund.balance(&bob), 50_000_000);
        assert_eq!(fund.aum(), 300_000_000);
        assert_eq!(fund.total_supply(), 150_000_000);
        assert_eq!(fund.nav(), 20_000_000); // NAV neutral after deposit
    }

    #[test]
    fn withdraw_proportional() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);

        // Backend sends 100 USDC to fund and records via add_tenant_fee → NAV = 2.0
        usdc_mint(&s, &s.fund_id, 100_000_000);
        fund.add_tenant_fee(&100_000_000);

        // Withdraw all 100 MUTAV at NAV=2 → receive 200 USDC
        fund.withdraw_investor(&investor, &100_000_000);

        assert_eq!(fund.total_supply(), 0);
        assert_eq!(fund.aum(), 0);
        assert_eq!(fund.balance(&investor), 0);
    }

    #[test]
    fn mgmt_fee_reduces_nav() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);

        // advance ledger timestamp by 31 days
        s.env.ledger().with_mut(|l| l.timestamp = 31 * 24 * 60 * 60);

        fund.charge_mgmt_fee();

        // 1% of 100 USDC = 1 USDC fee → AUM = 99 USDC, supply unchanged
        assert_eq!(fund.aum(), 99_000_000);
        assert_eq!(fund.total_supply(), 100_000_000);
        assert_eq!(
            token::Client::new(&s.env, &s.usdc_id).balance(&s.protocol),
            1_000_000
        );
    }

    #[test]
    fn cover_default_reduces_nav() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);
        let landlord = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);

        fund.cover_default(&10_000_000, &landlord);

        assert_eq!(fund.aum(), 90_000_000);
        assert_eq!(fund.total_supply(), 100_000_000);
        assert_eq!(fund.nav(), 9_000_000); // 0.9 USDC/MUTAV
    }
}
