#![no_std]
#![allow(deprecated)] // events().publish() is deprecated in favour of #[contractevent]; migrate later

use soroban_sdk::{
    contract, contractimpl, contracttype,
    symbol_short,
    token::{self, TokenInterface},
    Address, Env, IntoVal, MuxedAddress, String, Symbol, Val, Vec,
};

// 30 days in seconds — minimum interval between management fee charges
const MIN_FEE_INTERVAL: u64 = 30 * 24 * 60 * 60;
// Weekly exit cap: 2.5% of AUM per week
const WEEK_SECONDS: u64 = 7 * 24 * 60 * 60;
const EXIT_CAP_NUM: i128 = 25;
const EXIT_CAP_DEN: i128 = 1000;

// ── storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
enum DataKey {
    // fund accounting (instance storage — always loaded)
    Admin,
    ProtocolAddr,
    UsdcToken,
    ClassicWallet,
    RegistryContract,
    Aum,
    TotalSupply,
    LastFeeTimestamp,
    // weekly exit cap (instance storage)
    WeeklyEpoch,
    WeeklyExitUsed,
    // token metadata (instance storage)
    TokenMeta,
    // token accounting (persistent storage — per-address)
    Balance(Address),
    Allowance(AllowanceKey),
    // redemption queue (persistent storage)
    PendingRedemption(Address), // mutav locked, awaiting process_redemptions
    ReadyRedemption(Address),   // usdc owed after process_redemptions, awaiting fulfill
    RedemptionQueue,            // Vec<Address> FIFO
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

fn get_classic_wallet(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::ClassicWallet).expect("not initialized")
}

fn get_last_fee_ts(e: &Env) -> u64 {
    e.storage().instance().get(&DataKey::LastFeeTimestamp).unwrap_or(0)
}

fn set_last_fee_ts(e: &Env, v: u64) {
    e.storage().instance().set(&DataKey::LastFeeTimestamp, &v);
}

fn get_weekly_epoch(e: &Env) -> u64 {
    e.storage().instance().get(&DataKey::WeeklyEpoch).unwrap_or(0)
}

fn get_weekly_exit_used(e: &Env) -> i128 {
    e.storage().instance().get(&DataKey::WeeklyExitUsed).unwrap_or(0)
}

fn get_redemption_queue(e: &Env) -> Vec<Address> {
    e.storage()
        .persistent()
        .get(&DataKey::RedemptionQueue)
        .unwrap_or_else(|| Vec::new(e))
}

fn get_pending_redemption(e: &Env, addr: &Address) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::PendingRedemption(addr.clone()))
        .unwrap_or(0)
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
        .unwrap_or(AllowanceValue { amount: 0, expiration_ledger: 0 })
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

// If a registry contract is configured, assert the imobiliária is approved.
// Skipped when registry is not set (useful during tests and initial deploy).
fn check_imobiliaria_if_registry_set(e: &Env, imobiliaria: &Address) {
    if let Some(registry) =
        e.storage().instance().get::<_, Address>(&DataKey::RegistryContract)
    {
        let mut args: Vec<Val> = Vec::new(e);
        args.push_back(imobiliaria.into_val(e));
        let approved: bool = e.invoke_contract(
            &registry,
            &Symbol::new(e, "is_approved"),
            args,
        );
        assert!(approved, "imobiliaria not approved");
    }
}

// ── NAV math ──────────────────────────────────────────────────────────────────
//
// All USDC amounts are in micro-USDC (7 decimal places, same as Stellar stroops).
// All MUTAV amounts are also 7 decimal places.
//
// NAV  = AUM / supply                        (USDC units per MUTAV unit)
// Mint = amount_usdc * supply / aum           (proportional; 1:1 when supply == 0)
// Burn = mutav_amount * aum / supply          (proportional redemption)
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
        classic_wallet: Address,
    ) {
        if e.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage().instance().set(&DataKey::ProtocolAddr, &protocol_addr);
        e.storage().instance().set(&DataKey::UsdcToken, &usdc_token);
        e.storage().instance().set(&DataKey::ClassicWallet, &classic_wallet);
        e.storage().instance().set(
            &DataKey::TokenMeta,
            &TokenMeta {
                decimal: 7,
                name: String::from_str(&e, "MUTAV"),
                symbol: String::from_str(&e, "MUTAV"),
            },
        );
    }

    // ── on-ramp ───────────────────────────────────────────────────────────────

    /// Called by the backend after Etherfuse confirms USDC in the admin wallet.
    /// Pulls amount_usdc from the admin, splits 20% → protocol and 80% → classic_wallet,
    /// and records the 80% as AUM (it will become TESOURO via Etherfuse eYield).
    pub fn receive_payment(e: Env, imobiliaria: Address, amount_usdc: i128) {
        require_admin(&e);
        assert!(amount_usdc > 0, "amount must be positive");

        check_imobiliaria_if_registry_set(&e, &imobiliaria);

        let usdc = token::Client::new(&e, &get_usdc_token(&e));
        let admin = get_admin(&e);

        usdc.transfer(&admin, &e.current_contract_address(), &amount_usdc);

        let protocol_cut = amount_usdc / 5;              // 20%
        let fund_portion = amount_usdc - protocol_cut;   // 80%

        usdc.transfer(&e.current_contract_address(), &get_protocol_addr(&e), &protocol_cut);
        usdc.transfer(&e.current_contract_address(), &get_classic_wallet(&e), &fund_portion);

        set_aum(&e, get_aum(&e) + fund_portion);

        e.events().publish(
            (symbol_short!("rcv_pay"), imobiliaria),
            (amount_usdc, protocol_cut, fund_portion),
        );
    }

    // ── investor operations ───────────────────────────────────────────────────

    /// Investor deposits USDC and receives MUTAV tokens at the current NAV.
    pub fn deposit_investor(e: Env, investor: Address, amount_usdc: i128) {
        investor.require_auth();
        assert!(amount_usdc > 0, "amount must be positive");

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

    /// Queue a redemption request. Locks mutav_amount from the investor's balance.
    /// NAV is NOT calculated here — it is calculated at process_redemptions time,
    /// ensuring the investor exits at the NAV on the actual execution date.
    pub fn request_redemption(e: Env, investor: Address, mutav_amount: i128) {
        investor.require_auth();
        assert!(mutav_amount > 0, "amount must be positive");

        let bal = balance_of(&e, &investor);
        assert!(bal >= mutav_amount, "insufficient balance");

        write_balance(&e, &investor, bal - mutav_amount);

        let key = DataKey::PendingRedemption(investor.clone());
        let existing: i128 = e.storage().persistent().get(&key).unwrap_or(0);

        // First request: add to FIFO queue. Subsequent requests: accumulate.
        if existing == 0 {
            let mut queue = get_redemption_queue(&e);
            queue.push_back(investor.clone());
            e.storage().persistent().set(&DataKey::RedemptionQueue, &queue);
            e.storage()
                .persistent()
                .extend_ttl(&DataKey::RedemptionQueue, 518_400, 518_400);
        }

        let new_pending = existing + mutav_amount;
        e.storage().persistent().set(&key, &new_pending);
        e.storage().persistent().extend_ttl(&key, 518_400, 518_400);

        e.events().publish(
            (symbol_short!("req_rdmpt"), investor.clone()),
            (mutav_amount, new_pending),
        );
    }

    /// Cancel a pending (not yet processed) redemption request.
    /// Restores the locked MUTAV to the investor's balance.
    pub fn cancel_redemption(e: Env, investor: Address) {
        investor.require_auth();

        let key = DataKey::PendingRedemption(investor.clone());
        let pending: i128 =
            e.storage().persistent().get(&key).expect("no pending redemption");
        assert!(pending > 0, "no pending redemption");

        write_balance(&e, &investor, balance_of(&e, &investor) + pending);
        e.storage().persistent().remove(&key);

        // Remove investor from the queue
        let queue = get_redemption_queue(&e);
        let mut new_queue: Vec<Address> = Vec::new(&e);
        for i in 0..queue.len() {
            let addr = queue.get_unchecked(i);
            if addr != investor {
                new_queue.push_back(addr);
            }
        }
        e.storage().persistent().set(&DataKey::RedemptionQueue, &new_queue);
        if new_queue.len() > 0 {
            e.storage()
                .persistent()
                .extend_ttl(&DataKey::RedemptionQueue, 518_400, 518_400);
        }

        e.events().publish(
            (symbol_short!("cncl_rdmt"), investor.clone()),
            (pending,),
        );
    }

    /// Process the redemption queue up to the weekly 2.5% AUM exit cap.
    /// Burns MUTAV at the CURRENT NAV for each investor processed — not at
    /// request time — so investors exit at the NAV on the execution date.
    /// Returns the total USDC the backend must source (off-ramp TESOURO) and
    /// deposit into the contract before calling fulfill_redemption.
    pub fn process_redemptions(e: Env) -> i128 {
        require_admin(&e);

        // Roll over weekly epoch if needed
        let current_epoch = e.ledger().timestamp() / WEEK_SECONDS;
        if current_epoch > get_weekly_epoch(&e) {
            e.storage().instance().set(&DataKey::WeeklyEpoch, &current_epoch);
            e.storage().instance().set(&DataKey::WeeklyExitUsed, &0i128);
        }

        let mut aum = get_aum(&e);
        let mut supply = get_supply(&e);

        if supply == 0 {
            return 0;
        }

        let cap = aum * EXIT_CAP_NUM / EXIT_CAP_DEN;
        let already_used = get_weekly_exit_used(&e);
        let mut available = cap - already_used;

        if available <= 0 {
            return 0;
        }

        let queue = get_redemption_queue(&e);
        let mut first_unprocessed = queue.len(); // default: all processed
        let mut exit_used_delta: i128 = 0;
        let mut total_usdc: i128 = 0;

        for i in 0..queue.len() {
            let investor = queue.get_unchecked(i);
            let mutav = get_pending_redemption(&e, &investor);

            if mutav == 0 {
                // Stale entry — skip and let it fall off the queue
                continue;
            }

            let usdc_out = calc_redeem(mutav, aum, supply);

            if usdc_out > available {
                first_unprocessed = i;
                break;
            }

            // Burn MUTAV at current NAV
            supply -= mutav;
            aum -= usdc_out;
            available -= usdc_out;
            exit_used_delta += usdc_out;
            total_usdc += usdc_out;

            e.storage()
                .persistent()
                .remove(&DataKey::PendingRedemption(investor.clone()));

            let ready_key = DataKey::ReadyRedemption(investor.clone());
            e.storage().persistent().set(&ready_key, &usdc_out);
            e.storage().persistent().extend_ttl(&ready_key, 518_400, 518_400);

            e.events().publish(
                (symbol_short!("rdy_rdmpt"), investor.clone()),
                (mutav, usdc_out),
            );
        }

        set_aum(&e, aum);
        set_supply(&e, supply);
        e.storage()
            .instance()
            .set(&DataKey::WeeklyExitUsed, &(already_used + exit_used_delta));

        // Rebuild queue keeping only unprocessed entries
        let mut new_queue: Vec<Address> = Vec::new(&e);
        for i in first_unprocessed..queue.len() {
            new_queue.push_back(queue.get_unchecked(i));
        }
        e.storage().persistent().set(&DataKey::RedemptionQueue, &new_queue);
        if new_queue.len() > 0 {
            e.storage()
                .persistent()
                .extend_ttl(&DataKey::RedemptionQueue, 518_400, 518_400);
        }

        total_usdc
    }

    /// Pay out a processed investor. The backend must have deposited the USDC
    /// returned by process_redemptions into the contract before calling this.
    pub fn fulfill_redemption(e: Env, investor: Address) {
        require_admin(&e);

        let key = DataKey::ReadyRedemption(investor.clone());
        let usdc_out: i128 =
            e.storage().persistent().get(&key).expect("no ready redemption");
        assert!(usdc_out > 0, "nothing to fulfill");

        token::Client::new(&e, &get_usdc_token(&e)).transfer(
            &e.current_contract_address(),
            &investor,
            &usdc_out,
        );

        e.storage().persistent().remove(&key);

        e.events().publish(
            (symbol_short!("fulfill"), investor.clone()),
            (usdc_out,),
        );
    }

    // ── admin fund operations ─────────────────────────────────────────────────

    /// Manual AUM credit — for admin adjustments. In production, prefer
    /// receive_payment for on-ramp flows.
    pub fn add_tenant_fee(e: Env, amount_usdc: i128) {
        require_admin(&e);
        assert!(amount_usdc > 0, "amount must be positive");
        set_aum(&e, get_aum(&e) + amount_usdc);
        e.events().publish((symbol_short!("fee_in"),), (amount_usdc,));
    }

    /// Record incoming yield from tokenized treasury. Increases AUM → NAV increases.
    pub fn add_yield(e: Env, amount_usdc: i128) {
        require_admin(&e);
        assert!(amount_usdc > 0, "amount must be positive");
        set_aum(&e, get_aum(&e) + amount_usdc);
        e.events().publish((symbol_short!("yield_in"),), (amount_usdc,));
    }

    /// Charge the 1%/month management fee. Enforces a 30-day minimum interval.
    pub fn charge_mgmt_fee(e: Env) {
        require_admin(&e);

        let now = e.ledger().timestamp();
        let last = get_last_fee_ts(&e);
        assert!(now >= last + MIN_FEE_INTERVAL, "management fee already charged this period");

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

    /// Cover a default when USDC is available in the contract (e.g. from investor deposits).
    /// For defaults covered via TESOURO off-ramp, use record_offchain_payout instead.
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

    /// Record a payout that happened off-chain via the TESOURO → PIX path.
    /// Decrements AUM without moving USDC through the contract, since the
    /// off-ramp requires a Stellar Classic tx with MEMO (Soroban cannot send memos).
    pub fn record_offchain_payout(e: Env, amount_usdc: i128) {
        require_admin(&e);
        assert!(amount_usdc > 0, "amount must be positive");

        let aum = get_aum(&e);
        assert!(aum >= amount_usdc, "insufficient AUM");

        set_aum(&e, aum - amount_usdc);

        e.events().publish((symbol_short!("offchain"),), (amount_usdc,));
    }

    /// Bump TTL for all instance storage. Backend should call this every ~25 days.
    pub fn extend_ttl(e: Env) {
        e.storage().instance().extend_ttl(518_400, 518_400);
    }

    // ── admin config ──────────────────────────────────────────────────────────

    /// Set the Registry contract address to enable on-chain imobiliária approval checks.
    pub fn set_registry(e: Env, registry: Address) {
        require_admin(&e);
        e.storage().instance().set(&DataKey::RegistryContract, &registry);
    }

    pub fn set_classic_wallet(e: Env, wallet: Address) {
        require_admin(&e);
        e.storage().instance().set(&DataKey::ClassicWallet, &wallet);
    }

    pub fn set_admin(e: Env, new_admin: Address) {
        require_admin(&e);
        e.storage().instance().set(&DataKey::Admin, &new_admin);
    }

    // ── views ─────────────────────────────────────────────────────────────────

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

    pub fn pending_redemption(e: Env, investor: Address) -> i128 {
        get_pending_redemption(&e, &investor)
    }

    pub fn ready_redemption(e: Env, investor: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&DataKey::ReadyRedemption(investor))
            .unwrap_or(0)
    }

    pub fn queue_len(e: Env) -> u32 {
        get_redemption_queue(&e).len()
    }

    /// How much USDC can still exit this week before the 2.5% cap is hit.
    pub fn weekly_exit_available(e: Env) -> i128 {
        let cap = get_aum(&e) * EXIT_CAP_NUM / EXIT_CAP_DEN;
        let remaining = cap - get_weekly_exit_used(&e);
        if remaining < 0 { 0 } else { remaining }
    }
}

// ── SEP-0041 token interface ──────────────────────────────────────────────────

#[contractimpl]
impl TokenInterface for Fund {
    fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        let a = get_allowance(&e, &from, &spender);
        if a.expiration_ledger < e.ledger().sequence() {
            return 0;
        }
        a.amount
    }

    fn approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
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

    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
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
        let meta: TokenMeta =
            e.storage().instance().get(&DataKey::TokenMeta).expect("not initialized");
        meta.name
    }

    fn symbol(e: Env) -> String {
        let meta: TokenMeta =
            e.storage().instance().get(&DataKey::TokenMeta).expect("not initialized");
        meta.symbol
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Env,
    };

    struct Setup {
        env: Env,
        admin: Address,
        protocol: Address,
        classic_wallet: Address,
        fund_id: Address,
        usdc_id: Address,
    }

    fn setup() -> Setup {
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);
        let protocol = Address::generate(&e);
        let classic_wallet = Address::generate(&e);

        let usdc_id = e.register_stellar_asset_contract_v2(admin.clone()).address();
        let fund_id = e.register(Fund, ());

        FundClient::new(&e, &fund_id)
            .initialize(&admin, &protocol, &usdc_id, &classic_wallet);

        Setup { env: e, admin, protocol, classic_wallet, fund_id, usdc_id }
    }

    fn usdc_mint(s: &Setup, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&s.env, &s.usdc_id).mint(to, &amount);
    }

    // ── existing flow tests ───────────────────────────────────────────────────

    #[test]
    fn first_deposit_is_one_to_one() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
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

        usdc_mint(&s, &s.fund_id, 100_000_000);
        fund.add_tenant_fee(&100_000_000);
        assert_eq!(fund.nav(), 20_000_000); // NAV = 2.0

        usdc_mint(&s, &bob, 100_000_000);
        fund.deposit_investor(&bob, &100_000_000);

        assert_eq!(fund.balance(&bob), 50_000_000);
        assert_eq!(fund.aum(), 300_000_000);
        assert_eq!(fund.total_supply(), 150_000_000);
        assert_eq!(fund.nav(), 20_000_000);
    }

    #[test]
    fn mgmt_fee_reduces_nav() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);

        s.env.ledger().with_mut(|l| l.timestamp = 31 * 24 * 60 * 60);

        fund.charge_mgmt_fee();

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

    // ── on-ramp tests ─────────────────────────────────────────────────────────

    #[test]
    fn receive_payment_splits_20_80() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let imob = Address::generate(&s.env);
        let usdc = token::Client::new(&s.env, &s.usdc_id);

        usdc_mint(&s, &s.admin, 100_000_000); // 10 USDC
        fund.receive_payment(&imob, &100_000_000);

        assert_eq!(usdc.balance(&s.protocol), 20_000_000);      // 20%
        assert_eq!(usdc.balance(&s.classic_wallet), 80_000_000); // 80%
        assert_eq!(usdc.balance(&s.fund_id), 0);                 // nothing stays in contract
        assert_eq!(fund.aum(), 80_000_000);                      // AUM = 80%
    }

    // ── redemption queue tests ────────────────────────────────────────────────

    #[test]
    fn request_redemption_locks_mutav() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);

        fund.request_redemption(&investor, &60_000_000);

        assert_eq!(fund.balance(&investor), 40_000_000);
        assert_eq!(fund.pending_redemption(&investor), 60_000_000);
        assert_eq!(fund.queue_len(), 1);
    }

    #[test]
    fn nav_locks_at_process_time_not_request_time() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        // Investor deposits 100 USDC → 100M MUTAV at NAV = 1.0
        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        assert_eq!(fund.nav(), 10_000_000); // 1.0

        // Investor queues 1M MUTAV (1% of supply) — no burn yet, no NAV calculation.
        // At NAV=1.0 this would be worth 1M USDC if processed now.
        fund.request_redemption(&investor, &1_000_000);
        assert_eq!(fund.balance(&investor), 99_000_000);
        assert_eq!(fund.pending_redemption(&investor), 1_000_000);

        // Yield arrives: AUM doubles → NAV = 2.0
        usdc_mint(&s, &s.fund_id, 100_000_000);
        fund.add_yield(&100_000_000);
        assert_eq!(fund.nav(), 20_000_000); // 2.0

        // cap = 2.5% of AUM(200M) = 5M USDC; 1M MUTAV @ NAV=2.0 = 2M USDC → fits ✓
        // NAV locked at request time would yield 1M USDC.
        // NAV locked at execution time yields 2M USDC — this is what we assert.
        let total_needed = fund.process_redemptions();
        assert_eq!(total_needed, 2_000_000);
        assert_eq!(fund.ready_redemption(&investor), 2_000_000);
        assert_eq!(fund.queue_len(), 0);

        // Backend deposits 2M USDC from off-ramp and fulfills
        usdc_mint(&s, &s.fund_id, 2_000_000);
        fund.fulfill_redemption(&investor);

        assert_eq!(
            token::Client::new(&s.env, &s.usdc_id).balance(&investor),
            2_000_000
        );
        assert_eq!(fund.ready_redemption(&investor), 0);
    }

    #[test]
    fn weekly_cap_limits_queue() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let alice = Address::generate(&s.env);
        let bob = Address::generate(&s.env);

        // alice deposits 1 USDC, bob deposits 99 USDC → AUM = 100 USDC
        usdc_mint(&s, &alice, 10_000_000);
        usdc_mint(&s, &bob, 990_000_000);
        fund.deposit_investor(&alice, &10_000_000);
        fund.deposit_investor(&bob, &990_000_000);
        // AUM = 100 USDC, cap = 2.5% = 2.5 USDC

        fund.request_redemption(&alice, &10_000_000);  // 1 USDC → fits cap
        fund.request_redemption(&bob, &990_000_000);   // 99 USDC → exceeds cap

        let total = fund.process_redemptions();

        // Only alice fits within the 2.5 USDC cap
        assert_eq!(total, 10_000_000);
        assert_eq!(fund.ready_redemption(&alice), 10_000_000);
        assert_eq!(fund.pending_redemption(&bob), 990_000_000);
        assert_eq!(fund.queue_len(), 1); // bob still waiting
    }

    #[test]
    fn cancel_redemption_restores_balance() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &100_000_000);

        assert_eq!(fund.balance(&investor), 0);
        assert_eq!(fund.queue_len(), 1);

        fund.cancel_redemption(&investor);

        assert_eq!(fund.balance(&investor), 100_000_000);
        assert_eq!(fund.pending_redemption(&investor), 0);
        assert_eq!(fund.queue_len(), 0);
    }

    #[test]
    fn record_offchain_payout_reduces_aum() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);

        fund.record_offchain_payout(&10_000_000);

        assert_eq!(fund.aum(), 90_000_000);
        assert_eq!(fund.total_supply(), 100_000_000);
        assert_eq!(fund.nav(), 9_000_000);
    }

    #[test]
    fn weekly_epoch_resets_used_cap() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let alice = Address::generate(&s.env);
        let bob = Address::generate(&s.env);

        usdc_mint(&s, &alice, 10_000_000);
        usdc_mint(&s, &bob, 10_000_000);
        fund.deposit_investor(&alice, &10_000_000);
        fund.deposit_investor(&bob, &10_000_000);
        // AUM = 20 USDC, cap = 0.5 USDC per week

        fund.request_redemption(&alice, &10_000_000); // 1 USDC → exceeds 0.5 cap
        let total_week1 = fund.process_redemptions();
        assert_eq!(total_week1, 0); // cap too small for alice

        // Deposit more so alice fits the cap
        usdc_mint(&s, &alice, 390_000_000);
        fund.deposit_investor(&alice, &390_000_000);
        // AUM = 420 USDC, cap = 10.5 USDC — alice's 1 USDC fits now
        // But alice's pending is 10_000_000 (from 20 USDC AUM supply, she had 10M MUTAV)
        // After deposit her pending MUTAV is still 10_000_000 from earlier request

        // Advance one week
        s.env.ledger().with_mut(|l| l.timestamp = WEEK_SECONDS + 1);

        let total_week2 = fund.process_redemptions();
        // alice's 10M MUTAV at current NAV should fit the new cap
        assert!(total_week2 > 0);
        assert_eq!(fund.pending_redemption(&alice), 0);
    }
}
