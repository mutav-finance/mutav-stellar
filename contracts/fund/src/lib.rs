#![no_std]
#![allow(deprecated)] // events().publish() is deprecated in favour of #[contractevent]; migrate later
#![allow(clippy::too_many_arguments)] // initialize takes many config params; acceptable for Soroban contracts

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    token::{self, TokenInterface},
    Address, Env, IntoVal, MuxedAddress, String, Symbol, Val, Vec,
};

// 30 days in seconds — minimum interval between management fee charges
const MIN_FEE_INTERVAL: u64 = 30 * 24 * 60 * 60;
const WEEK_SECONDS: u64 = 7 * 24 * 60 * 60;
// Maximum queue entries examined per process_redemptions call.
//
// Two independent Soroban limits constrain this value:
//
// 1. Read footprint (100 ledger entries): fixed overhead ~14, leaving ~86.
//    Worst case per examined entry = 2 slots → ceiling 86/2 = 43, rounded to 40.
//
// 2. Write budget (50 write entries): each PROCESSED investor costs 3 writes
//    (remove PendingRedemption + write ReadyRedemption + TTL ReadyRedemption).
//    Each DEFERRED investor costs 1 write (TTL extend on PendingRedemption).
//    With 40 examined: 3p + (40−p) + ~7 overhead ≤ 50 → p ≤ 1 processed per call.
//    When fewer are examined (small queue), more can be processed: ceiling ~14 when
//    none are deferred (3×14 + 7 = 49).
//
// Practical implication: with a full queue, only ~1–2 investors are processed per call
// regardless of the exit cap. The operator may need multiple weekly calls to drain
// large queues, or adjust the cap between calls to allow larger batches.
const MAX_QUEUE_BATCH: u32 = 40;

// ── storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
enum DataKey {
    // fund governance (instance storage)
    Admin,    // owner: cold wallet, governance-only
    Operator, // hot wallet, daily operations
    // fund accounting (instance storage — always loaded)
    ProtocolAddr,
    UsdcToken,
    ClassicWallet,
    RegistryContract,
    Aum,
    TotalSupply,
    LastFeeTimestamp,
    // fund config (instance storage — set at initialize, read-only after)
    ExitCapBps,
    MgmtFeeBps,
    RedemptionFeeBps,
    ProtocolFeeBps,    // share of receive_payment sent to protocol wallet
    MaxAumIncreaseBps, // per-call cap on add_yield / add_tenant_fee
    // weekly exit cap (instance storage)
    WeeklyEpoch,
    WeeklyExitUsed,
    // token metadata (instance storage)
    TokenMeta,
    // token accounting (persistent storage — per-address)
    Balance(Address),
    Allowance(AllowanceKey),
    // emergency pause
    Paused,
    // two-step admin transfer
    PendingAdmin,
    // fulfill window config
    FulfillWindowSeconds,
    // redemption queue (persistent storage)
    PendingRedemption(Address), // mutav locked, awaiting process_redemptions
    ReadyRedemption(Address),   // ReadyRedemptionData after process_redemptions
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
struct ReadyRedemptionData {
    usdc_gross: i128,
    mutav_burned: i128,
    deadline: u64,
    fee_bps: u32, // snapshot of redemption_fee_bps at process time — immune to later changes
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
    e.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("not initialized")
}

fn get_operator(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&DataKey::Operator)
        .expect("not initialized")
}

fn get_aum(e: &Env) -> i128 {
    e.storage().instance().get(&DataKey::Aum).unwrap_or(0)
}

fn set_aum(e: &Env, v: i128) {
    e.storage().instance().set(&DataKey::Aum, &v);
}

fn get_supply(e: &Env) -> i128 {
    e.storage()
        .instance()
        .get(&DataKey::TotalSupply)
        .unwrap_or(0)
}

fn set_supply(e: &Env, v: i128) {
    e.storage().instance().set(&DataKey::TotalSupply, &v);
}

fn get_protocol_addr(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&DataKey::ProtocolAddr)
        .expect("not initialized")
}

fn get_usdc_token(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&DataKey::UsdcToken)
        .expect("not initialized")
}

fn get_classic_wallet(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&DataKey::ClassicWallet)
        .expect("not initialized")
}

fn get_exit_cap_bps(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::ExitCapBps)
        .expect("not initialized")
}

fn get_mgmt_fee_bps(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::MgmtFeeBps)
        .expect("not initialized")
}

fn get_redemption_fee_bps(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::RedemptionFeeBps)
        .expect("not initialized")
}

fn get_protocol_fee_bps(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::ProtocolFeeBps)
        .expect("not initialized")
}

fn get_max_aum_increase_bps(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::MaxAumIncreaseBps)
        .expect("not initialized")
}

fn get_fulfill_window(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get(&DataKey::FulfillWindowSeconds)
        .expect("not initialized")
}

fn get_last_fee_ts(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get(&DataKey::LastFeeTimestamp)
        .unwrap_or(0)
}

fn set_last_fee_ts(e: &Env, v: u64) {
    e.storage().instance().set(&DataKey::LastFeeTimestamp, &v);
}

fn get_weekly_epoch(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get(&DataKey::WeeklyEpoch)
        .unwrap_or(0)
}

fn get_weekly_exit_used(e: &Env) -> i128 {
    e.storage()
        .instance()
        .get(&DataKey::WeeklyExitUsed)
        .unwrap_or(0)
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
    e.storage().temporary().get(&key).unwrap_or(AllowanceValue {
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
    e.storage().temporary().set(
        &key,
        &AllowanceValue {
            amount,
            expiration_ledger,
        },
    );

    if amount > 0 {
        let current = e.ledger().sequence();
        assert!(
            expiration_ledger >= current,
            "expiration_ledger must be >= current ledger"
        );
        let live_for = expiration_ledger - current;
        e.storage().temporary().extend_ttl(&key, live_for, live_for);
    }
}

// Owner: cold wallet — governance and config changes only.
fn require_admin(e: &Env) {
    get_admin(e).require_auth();
}

// Operator: hot wallet — daily fund operations.
fn require_operator(e: &Env) {
    get_operator(e).require_auth();
}

fn require_not_paused(e: &Env) {
    assert!(
        !e.storage()
            .instance()
            .get::<_, bool>(&DataKey::Paused)
            .unwrap_or(false),
        "contract is paused"
    );
}

// If a registry contract is configured, assert the imobiliária is approved.
// Skipped when registry is not set (useful during tests and initial deploy).
fn check_imobiliaria_if_registry_set(e: &Env, imobiliaria: &Address) {
    if let Some(registry) = e
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::RegistryContract)
    {
        let mut args: Vec<Val> = Vec::new(e);
        args.push_back(imobiliaria.into_val(e));
        let approved: bool = e.invoke_contract(&registry, &Symbol::new(e, "is_approved"), args);
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
        assert!(
            aum > 0,
            "fund is insolvent: AUM is zero with non-zero supply"
        );
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
#[allow(clippy::too_many_arguments)]
impl Fund {
    /// Deploy and configure the fund. Called once.
    /// - `admin`: owner / cold wallet — governance only
    /// - `operator`: hot wallet — daily operations
    /// - `token_name` / `token_symbol`: e.g. "MTVL", "MTVM", "MTVH"
    /// - `exit_cap_bps`: weekly exit cap in basis points (250 = 2.5%; max 10_000)
    /// - `mgmt_fee_bps`: monthly management fee in basis points (100 = 1%; max 1_000)
    /// - `redemption_fee_bps`: fee on each payout (25 = 0.25%; max 1_000)
    /// - `protocol_fee_bps`: share of receive_payment sent to protocol wallet (2_000 = 20%; max 5_000)
    /// - `fulfill_window_seconds`: seconds the backend has to fulfill before investor can reclaim
    /// - `max_aum_increase_bps`: per-call cap on add_yield / add_tenant_fee (500 = 5%; max 10_000)
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        e: Env,
        admin: Address,
        operator: Address,
        protocol_addr: Address,
        usdc_token: Address,
        classic_wallet: Address,
        token_name: String,
        token_symbol: String,
        exit_cap_bps: u32,
        mgmt_fee_bps: u32,
        redemption_fee_bps: u32,
        protocol_fee_bps: u32,
        fulfill_window_seconds: u64,
        max_aum_increase_bps: u32,
    ) {
        assert!(exit_cap_bps <= 10_000, "exit_cap_bps exceeds 100%");
        assert!(mgmt_fee_bps <= 1_000, "mgmt_fee_bps exceeds 10%");
        assert!(
            redemption_fee_bps <= 1_000,
            "redemption_fee_bps exceeds 10%"
        );
        assert!(protocol_fee_bps <= 5_000, "protocol_fee_bps exceeds 50%");
        assert!(
            fulfill_window_seconds > 0,
            "fulfill_window_seconds must be positive"
        );
        assert!(
            max_aum_increase_bps > 0 && max_aum_increase_bps <= 10_000,
            "max_aum_increase_bps must be 1..10_000"
        );

        if e.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage().instance().set(&DataKey::Operator, &operator);
        e.storage()
            .instance()
            .set(&DataKey::ProtocolAddr, &protocol_addr);
        e.storage().instance().set(&DataKey::UsdcToken, &usdc_token);
        e.storage()
            .instance()
            .set(&DataKey::ClassicWallet, &classic_wallet);
        e.storage()
            .instance()
            .set(&DataKey::ExitCapBps, &exit_cap_bps);
        e.storage()
            .instance()
            .set(&DataKey::MgmtFeeBps, &mgmt_fee_bps);
        e.storage()
            .instance()
            .set(&DataKey::RedemptionFeeBps, &redemption_fee_bps);
        e.storage()
            .instance()
            .set(&DataKey::ProtocolFeeBps, &protocol_fee_bps);
        e.storage()
            .instance()
            .set(&DataKey::MaxAumIncreaseBps, &max_aum_increase_bps);
        e.storage()
            .instance()
            .set(&DataKey::FulfillWindowSeconds, &fulfill_window_seconds);
        e.storage().instance().set(
            &DataKey::TokenMeta,
            &TokenMeta {
                decimal: 7,
                name: token_name,
                symbol: token_symbol,
            },
        );
    }

    // ── on-ramp ───────────────────────────────────────────────────────────────

    /// Called by the backend after Etherfuse confirms USDC in the operator wallet.
    /// Pulls amount_usdc from the operator, splits 20% → protocol and 80% → classic_wallet,
    /// and records the 80% as AUM (it will become TESOURO via Etherfuse eYield).
    pub fn receive_payment(e: Env, imobiliaria: Address, amount_usdc: i128) {
        require_operator(&e);
        require_not_paused(&e);
        assert!(amount_usdc > 0, "amount must be positive");

        check_imobiliaria_if_registry_set(&e, &imobiliaria);

        let usdc = token::Client::new(&e, &get_usdc_token(&e));
        let operator = get_operator(&e);

        usdc.transfer(&operator, e.current_contract_address(), &amount_usdc);

        let protocol_cut = amount_usdc * get_protocol_fee_bps(&e) as i128 / 10_000;
        let fund_portion = amount_usdc - protocol_cut;

        usdc.transfer(
            &e.current_contract_address(),
            get_protocol_addr(&e),
            &protocol_cut,
        );
        usdc.transfer(
            &e.current_contract_address(),
            get_classic_wallet(&e),
            &fund_portion,
        );

        set_aum(&e, get_aum(&e) + fund_portion);

        e.events().publish(
            (symbol_short!("rcv_pay"), imobiliaria),
            (amount_usdc, protocol_cut, fund_portion),
        );
    }

    // ── investor operations ───────────────────────────────────────────────────

    /// Investor deposits USDC and receives MUTAV tokens at the current NAV.
    /// The full deposit is forwarded immediately to the Classic wallet to be
    /// converted to TESOURO via Etherfuse eYield — the contract retains no USDC.
    pub fn deposit_investor(e: Env, investor: Address, amount_usdc: i128) {
        investor.require_auth();
        require_not_paused(&e);
        assert!(amount_usdc > 0, "amount must be positive");

        let usdc = token::Client::new(&e, &get_usdc_token(&e));

        usdc.transfer(&investor, e.current_contract_address(), &amount_usdc);
        usdc.transfer(
            &e.current_contract_address(),
            get_classic_wallet(&e),
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
        require_not_paused(&e);
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
            e.storage()
                .persistent()
                .set(&DataKey::RedemptionQueue, &queue);
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
    /// O(1): the address stays in RedemptionQueue as a ghost (mutav==0);
    /// process_redemptions already skips and purges such entries.
    pub fn cancel_redemption(e: Env, investor: Address) {
        investor.require_auth();

        let key = DataKey::PendingRedemption(investor.clone());
        let pending: i128 = e
            .storage()
            .persistent()
            .get(&key)
            .expect("no pending redemption");
        assert!(pending > 0, "no pending redemption");

        write_balance(&e, &investor, balance_of(&e, &investor) + pending);
        e.storage().persistent().remove(&key);

        e.events()
            .publish((symbol_short!("cncl_rdmt"), investor.clone()), (pending,));
    }

    /// Process the redemption queue up to the weekly exit cap.
    /// Burns MUTAV at the CURRENT NAV for each investor processed — not at
    /// request time — so investors exit at the NAV on the execution date.
    /// Entries that exceed the remaining weekly cap are deferred (not skipped
    /// permanently): smaller requests behind them are still processed if they fit.
    /// Returns the total USDC the backend must source (off-ramp TESOURO) and
    /// deposit into the contract before calling fulfill_redemption.
    pub fn process_redemptions(e: Env) -> i128 {
        require_operator(&e);
        require_not_paused(&e);

        // Roll over weekly epoch if needed
        let current_epoch = e.ledger().timestamp() / WEEK_SECONDS;
        if current_epoch > get_weekly_epoch(&e) {
            e.storage()
                .instance()
                .set(&DataKey::WeeklyEpoch, &current_epoch);
            e.storage().instance().set(&DataKey::WeeklyExitUsed, &0i128);
        }

        let mut aum = get_aum(&e);
        let mut supply = get_supply(&e);

        if supply == 0 {
            return 0;
        }

        let cap = aum * get_exit_cap_bps(&e) as i128 / 10_000;
        let already_used = get_weekly_exit_used(&e);
        let mut available = cap - already_used;

        if available <= 0 {
            return 0;
        }

        let queue = get_redemption_queue(&e);
        let mut new_queue: Vec<Address> = Vec::new(&e);
        let mut exit_used_delta: i128 = 0;
        let mut total_usdc: i128 = 0;
        let mut examined: u32 = 0;

        for i in 0..queue.len() {
            let investor = queue.get_unchecked(i);

            if examined >= MAX_QUEUE_BATCH {
                // Batch cap reached — carry remaining entries forward unexamined
                new_queue.push_back(investor);
                continue;
            }
            examined += 1;

            let mutav = get_pending_redemption(&e, &investor);

            if mutav == 0 {
                // Ghost from cancel_redemption — purge from queue
                continue;
            }

            let usdc_out = calc_redeem(mutav, aum, supply);

            if usdc_out > available {
                // Exceeds remaining cap this week — defer; smaller entries behind still run
                let pk = DataKey::PendingRedemption(investor.clone());
                e.storage().persistent().extend_ttl(&pk, 518_400, 518_400);
                new_queue.push_back(investor);
                continue;
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
            let deadline = e.ledger().timestamp() + get_fulfill_window(&e);
            e.storage().persistent().set(
                &ready_key,
                &ReadyRedemptionData {
                    usdc_gross: usdc_out,
                    mutav_burned: mutav,
                    deadline,
                    fee_bps: get_redemption_fee_bps(&e),
                },
            );
            e.storage()
                .persistent()
                .extend_ttl(&ready_key, 518_400, 518_400);

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

        e.storage()
            .persistent()
            .set(&DataKey::RedemptionQueue, &new_queue);
        if !new_queue.is_empty() {
            e.storage()
                .persistent()
                .extend_ttl(&DataKey::RedemptionQueue, 518_400, 518_400);
        }

        total_usdc
    }

    /// Pay out a processed investor. The backend must have deposited the USDC
    /// returned by process_redemptions into the contract before calling this.
    /// Deducts the redemption fee (redemption_fee_bps) and forwards it to the
    /// protocol wallet; the remainder goes to the investor.
    /// Panics if the fulfill window has already expired — use reclaim_expired_redemption instead.
    pub fn fulfill_redemption(e: Env, investor: Address) {
        require_operator(&e);
        require_not_paused(&e);

        let key = DataKey::ReadyRedemption(investor.clone());
        let data: ReadyRedemptionData = e
            .storage()
            .persistent()
            .get(&key)
            .expect("no ready redemption");
        let gross = data.usdc_gross;
        assert!(gross > 0, "nothing to fulfill");
        assert!(
            e.ledger().timestamp() <= data.deadline,
            "fulfill window has expired; investor may reclaim"
        );

        let fee = gross * data.fee_bps as i128 / 10_000;
        let investor_amount = gross - fee;

        let usdc = token::Client::new(&e, &get_usdc_token(&e));

        usdc.transfer(&e.current_contract_address(), &investor, &investor_amount);

        if fee > 0 {
            usdc.transfer(&e.current_contract_address(), get_protocol_addr(&e), &fee);
        }

        e.storage().persistent().remove(&key);

        e.events().publish(
            (symbol_short!("fulfill"), investor.clone()),
            (investor_amount, fee),
        );
    }

    /// If the backend fails to call fulfill_redemption before the deadline,
    /// the investor can reclaim: their MUTAV is restored and AUM is credited back.
    pub fn reclaim_expired_redemption(e: Env, investor: Address) {
        investor.require_auth();

        let key = DataKey::ReadyRedemption(investor.clone());
        let data: ReadyRedemptionData = e
            .storage()
            .persistent()
            .get(&key)
            .expect("no ready redemption");

        assert!(
            e.ledger().timestamp() > data.deadline,
            "fulfill window has not expired yet"
        );

        // Restore MUTAV to investor and credit AUM back
        write_balance(&e, &investor, balance_of(&e, &investor) + data.mutav_burned);
        set_supply(&e, get_supply(&e) + data.mutav_burned);
        set_aum(&e, get_aum(&e) + data.usdc_gross);

        e.storage().persistent().remove(&key);

        e.events().publish(
            (symbol_short!("reclaim"), investor.clone()),
            (data.mutav_burned, data.usdc_gross),
        );
    }

    // ── operator fund operations ──────────────────────────────────────────────

    /// Manual AUM credit — for recording tenant fee income.
    /// Always enforces max_aum_increase_bps; when AUM is zero the cap is zero,
    /// so any positive amount is rejected — preventing free yield for the first depositor.
    pub fn add_tenant_fee(e: Env, amount_usdc: i128) {
        require_operator(&e);
        require_not_paused(&e);
        assert!(amount_usdc > 0, "amount must be positive");

        let aum = get_aum(&e);
        let max_increase = aum * get_max_aum_increase_bps(&e) as i128 / 10_000;
        assert!(
            amount_usdc <= max_increase,
            "AUM increase exceeds per-call cap"
        );

        set_aum(&e, aum + amount_usdc);
        e.events()
            .publish((symbol_short!("fee_in"),), (amount_usdc,));
    }

    /// Record incoming yield from tokenized treasury. Increases AUM → NAV increases.
    /// Always enforces max_aum_increase_bps; when AUM is zero the cap is zero,
    /// so any positive amount is rejected — preventing free yield for the first depositor.
    pub fn add_yield(e: Env, amount_usdc: i128) {
        require_operator(&e);
        require_not_paused(&e);
        assert!(amount_usdc > 0, "amount must be positive");

        let aum = get_aum(&e);
        let max_increase = aum * get_max_aum_increase_bps(&e) as i128 / 10_000;
        assert!(
            amount_usdc <= max_increase,
            "AUM increase exceeds per-call cap"
        );

        set_aum(&e, aum + amount_usdc);
        e.events()
            .publish((symbol_short!("yield_in"),), (amount_usdc,));
    }

    /// Charge the 1%/month management fee. Enforces a 30-day minimum interval.
    /// Purely accounting — decrements AUM and records the timestamp.
    /// The actual payment to the protocol wallet is made off-chain by the backend
    /// from the Classic wallet (all fund value lives as TESOURO, not USDC in this contract).
    pub fn charge_mgmt_fee(e: Env) {
        require_operator(&e);
        require_not_paused(&e);

        let now = e.ledger().timestamp();
        let last = get_last_fee_ts(&e);
        assert!(
            now >= last + MIN_FEE_INTERVAL,
            "management fee already charged this period"
        );

        let aum = get_aum(&e);
        let fee = aum * get_mgmt_fee_bps(&e) as i128 / 10_000;
        assert!(fee > 0, "AUM too small to charge fee");

        set_aum(&e, aum - fee);
        set_last_fee_ts(&e, now);

        e.events().publish((symbol_short!("mgmt_fee"),), (fee,));
    }

    /// Record a payout that happened off-chain via the TESOURO → PIX path.
    /// Decrements AUM without moving USDC through the contract, since the
    /// off-ramp requires a Stellar Classic tx with MEMO (Soroban cannot send memos).
    /// `destination` is logged for audit trail — it does not receive on-chain USDC.
    pub fn record_offchain_payout(e: Env, amount_usdc: i128, destination: Address) {
        require_operator(&e);
        require_not_paused(&e);
        assert!(amount_usdc > 0, "amount must be positive");

        let aum = get_aum(&e);
        assert!(aum >= amount_usdc, "insufficient AUM");

        set_aum(&e, aum - amount_usdc);

        e.events().publish(
            (symbol_short!("offchain"), destination.clone()),
            (amount_usdc,),
        );
    }

    /// Bump TTL for all instance storage. Backend should call this every ~25 days.
    pub fn extend_ttl(e: Env) {
        require_operator(&e);
        e.storage().instance().extend_ttl(518_400, 518_400);
    }

    /// Move idle USDC sitting in the contract back to the Classic wallet so it can
    /// resume earning yield via Etherfuse eYield.
    ///
    /// When does USDC get stranded here?
    /// The backend deposits USDC for a ready redemption, but the investor reclaims
    /// before fulfill_redemption is called. reclaim_expired_redemption restores the
    /// AUM accounting, so the USDC is already counted — only capital efficiency
    /// suffers. AUM is intentionally NOT changed by this function.
    ///
    /// The operator must ensure `amount` does not exceed idle USDC:
    ///   idle = usdc_token.balance(contract) − Σ usdc_gross of all ReadyRedemption entries
    /// Sweeping USDC reserved for pending fulfill_redemption calls will cause those
    /// calls to fail.
    pub fn sweep_usdc(e: Env, amount: i128) {
        require_operator(&e);
        assert!(amount > 0, "amount must be positive");

        let usdc = token::Client::new(&e, &get_usdc_token(&e));
        usdc.transfer(
            &e.current_contract_address(),
            &get_classic_wallet(&e),
            &amount,
        );

        e.events().publish((symbol_short!("sweep"),), (amount,));
    }

    // ── owner / governance operations ─────────────────────────────────────────

    /// Record a default payout. Purely accounting — decrements AUM and logs the
    /// destination address for on-chain audit trail.
    /// The actual payment to the landlord goes via TESOURO off-ramp from the
    /// Classic wallet (Stellar Classic tx with MEMO → Etherfuse → PIX).
    pub fn cover_default(e: Env, amount_usdc: i128, destination: Address) {
        require_admin(&e);
        assert!(amount_usdc > 0, "amount must be positive");

        let aum = get_aum(&e);
        assert!(aum >= amount_usdc, "insufficient AUM");

        set_aum(&e, aum - amount_usdc);

        e.events().publish(
            (symbol_short!("default"), destination.clone()),
            (amount_usdc,),
        );
    }

    /// Set the Registry contract address to enable on-chain imobiliária approval checks.
    pub fn set_registry(e: Env, registry: Address) {
        require_admin(&e);
        e.storage()
            .instance()
            .set(&DataKey::RegistryContract, &registry);
        e.events().publish((symbol_short!("set_reg"),), (registry,));
    }

    /// Remove the Registry contract, reverting to permissionless receive_payment.
    /// Use when the registry is deprecated or unavailable.
    pub fn remove_registry(e: Env) {
        require_admin(&e);
        e.storage().instance().remove(&DataKey::RegistryContract);
        e.events().publish((symbol_short!("rm_reg"),), ());
    }

    pub fn set_classic_wallet(e: Env, wallet: Address) {
        require_admin(&e);
        e.storage().instance().set(&DataKey::ClassicWallet, &wallet);
        e.events().publish((symbol_short!("set_wall"),), (wallet,));
    }

    /// Replace the operator address. Owner-only.
    pub fn set_operator(e: Env, new_operator: Address) {
        require_admin(&e);
        e.storage()
            .instance()
            .set(&DataKey::Operator, &new_operator);
        e.events()
            .publish((symbol_short!("set_op"),), (new_operator,));
    }

    pub fn set_exit_cap_bps(e: Env, value: u32) {
        require_admin(&e);
        assert!(
            value > 0 && value <= 10_000,
            "exit_cap_bps must be 1..10_000"
        );
        e.storage().instance().set(&DataKey::ExitCapBps, &value);
        e.events().publish((symbol_short!("set_exit"),), (value,));
    }

    pub fn set_mgmt_fee_bps(e: Env, value: u32) {
        require_admin(&e);
        assert!(value <= 1_000, "mgmt_fee_bps exceeds 10%");
        e.storage().instance().set(&DataKey::MgmtFeeBps, &value);
        e.events().publish((symbol_short!("set_mgmt"),), (value,));
    }

    pub fn set_redemption_fee_bps(e: Env, value: u32) {
        require_admin(&e);
        assert!(value <= 1_000, "redemption_fee_bps exceeds 10%");
        e.storage()
            .instance()
            .set(&DataKey::RedemptionFeeBps, &value);
        e.events().publish((symbol_short!("set_rdmf"),), (value,));
    }

    pub fn set_protocol_fee_bps(e: Env, value: u32) {
        require_admin(&e);
        assert!(value <= 5_000, "protocol_fee_bps exceeds 50%");
        e.storage()
            .instance()
            .set(&DataKey::ProtocolFeeBps, &value);
        e.events().publish((symbol_short!("set_prtf"),), (value,));
    }

    pub fn set_max_aum_increase_bps(e: Env, value: u32) {
        require_admin(&e);
        assert!(
            value > 0 && value <= 10_000,
            "max_aum_increase_bps must be 1..10_000"
        );
        e.storage()
            .instance()
            .set(&DataKey::MaxAumIncreaseBps, &value);
        e.events().publish((symbol_short!("set_maum"),), (value,));
    }

    pub fn set_fulfill_window(e: Env, seconds: u64) {
        require_admin(&e);
        assert!(seconds > 0, "fulfill_window_seconds must be positive");
        e.storage()
            .instance()
            .set(&DataKey::FulfillWindowSeconds, &seconds);
        e.events().publish((symbol_short!("set_fwin"),), (seconds,));
    }

    /// Pause or unpause all fund operations. Admin-only.
    /// When paused, deposits, redemptions, yield additions, and management fees are blocked.
    /// cancel_redemption and reclaim_expired_redemption remain available so investors can
    /// always recover their funds regardless of contract state.
    pub fn set_paused(e: Env, paused: bool) {
        require_admin(&e);
        e.storage().instance().set(&DataKey::Paused, &paused);
        e.events().publish((symbol_short!("set_paus"),), (paused,));
    }

    /// Step 1: current admin nominates a new admin address.
    pub fn propose_admin(e: Env, new_admin: Address) {
        require_admin(&e);
        e.storage()
            .instance()
            .set(&DataKey::PendingAdmin, &new_admin);
        e.events()
            .publish((symbol_short!("prop_adm"),), (new_admin,));
    }

    /// Step 2: nominated address must call this to complete the transfer.
    /// Prevents typos from locking out the contract permanently.
    pub fn accept_admin(e: Env) {
        let pending: Address = e
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .expect("no pending admin");
        pending.require_auth();
        e.storage().instance().set(&DataKey::Admin, &pending);
        e.storage().instance().remove(&DataKey::PendingAdmin);
        e.events().publish((symbol_short!("acc_adm"),), (pending,));
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
            .get::<_, ReadyRedemptionData>(&DataKey::ReadyRedemption(investor))
            .map(|d| d.usdc_gross)
            .unwrap_or(0)
    }

    /// Unix timestamp (seconds) by which the backend must call fulfill_redemption.
    /// Returns 0 if the investor has no ready redemption.
    /// If the current ledger timestamp is past this value, the investor can call
    /// reclaim_expired_redemption to recover their position.
    pub fn ready_redemption_deadline(e: Env, investor: Address) -> u64 {
        e.storage()
            .persistent()
            .get::<_, ReadyRedemptionData>(&DataKey::ReadyRedemption(investor))
            .map(|d| d.deadline)
            .unwrap_or(0)
    }

    /// Number of active (non-cancelled) entries in the redemption queue.
    /// Ghost entries left by cancel_redemption are excluded.
    pub fn queue_len(e: Env) -> u32 {
        let queue = get_redemption_queue(&e);
        let mut count = 0u32;
        for i in 0..queue.len() {
            if get_pending_redemption(&e, &queue.get_unchecked(i)) > 0 {
                count += 1;
            }
        }
        count
    }

    pub fn exit_cap_bps(e: Env) -> u32 {
        get_exit_cap_bps(&e)
    }

    pub fn mgmt_fee_bps(e: Env) -> u32 {
        get_mgmt_fee_bps(&e)
    }

    pub fn redemption_fee_bps(e: Env) -> u32 {
        get_redemption_fee_bps(&e)
    }

    pub fn protocol_fee_bps(e: Env) -> u32 {
        get_protocol_fee_bps(&e)
    }

    pub fn max_aum_increase_bps(e: Env) -> u32 {
        get_max_aum_increase_bps(&e)
    }

    pub fn operator(e: Env) -> Address {
        get_operator(&e)
    }

    pub fn admin(e: Env) -> Address {
        get_admin(&e)
    }

    pub fn paused(e: Env) -> bool {
        e.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Extend the TTL of an investor's balance entry so it doesn't expire.
    /// Permissionless — anyone (including the investor themselves) can call this.
    /// Needed for long-term holders who don't interact with the contract for 30+ days.
    pub fn extend_balance_ttl(e: Env, investor: Address) {
        let key = DataKey::Balance(investor);
        if e.storage().persistent().has(&key) {
            e.storage().persistent().extend_ttl(&key, 518_400, 518_400);
        }
    }

    /// Extend the TTL of an investor's pending or ready redemption entries, and of the
    /// queue itself. Permissionless — call this if a redemption is stuck in the queue for
    /// an extended period and there is a risk of the persistent storage expiring (~30 days
    /// of inactivity). Renewing the queue is essential: if it expires while the investor's
    /// entry is still alive, the entry becomes un-processable (cancel still works, but
    /// the investor loses their queue position).
    pub fn extend_redemption_ttl(e: Env, investor: Address) {
        let pending_key = DataKey::PendingRedemption(investor.clone());
        if e.storage().persistent().has(&pending_key) {
            e.storage()
                .persistent()
                .extend_ttl(&pending_key, 518_400, 518_400);
        }
        let ready_key = DataKey::ReadyRedemption(investor);
        if e.storage().persistent().has(&ready_key) {
            e.storage()
                .persistent()
                .extend_ttl(&ready_key, 518_400, 518_400);
        }
        if e.storage().persistent().has(&DataKey::RedemptionQueue) {
            e.storage()
                .persistent()
                .extend_ttl(&DataKey::RedemptionQueue, 518_400, 518_400);
        }
    }

    /// How much USDC can still exit this week before the exit cap is hit.
    /// Accounts for epoch rollover so the value is always fresh.
    pub fn weekly_exit_available(e: Env) -> i128 {
        let current_epoch = e.ledger().timestamp() / WEEK_SECONDS;
        let exit_used = if current_epoch > get_weekly_epoch(&e) {
            0i128 // new epoch — cap fully resets
        } else {
            get_weekly_exit_used(&e)
        };
        let cap = get_aum(&e) * get_exit_cap_bps(&e) as i128 / 10_000;
        let remaining = cap - exit_used;
        if remaining < 0 {
            0
        } else {
            remaining
        }
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
        e.events()
            .publish((symbol_short!("transfer"), from.clone()), (to_addr, amount));
    }

    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        assert!(amount > 0, "amount must be positive");

        let a = get_allowance(&e, &from, &spender);
        assert!(
            a.expiration_ledger >= e.ledger().sequence(),
            "allowance expired"
        );
        assert!(a.amount >= amount, "insufficient allowance");

        write_allowance(&e, &from, &spender, a.amount - amount, a.expiration_ledger);

        let bal = balance_of(&e, &from);
        assert!(bal >= amount, "insufficient balance");
        write_balance(&e, &from, bal - amount);
        write_balance(&e, &to, balance_of(&e, &to) + amount);

        e.events()
            .publish((symbol_short!("transfer"), from.clone()), (to, amount));
    }

    fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "amount must be positive");
        let bal = balance_of(&e, &from);
        assert!(bal >= amount, "insufficient balance");

        let supply = get_supply(&e);
        let aum = get_aum(&e);
        // Reduce AUM proportionally so NAV stays stable; without this, burning
        // MUTAV outside the redemption queue would inflate NAV for remaining holders.
        let aum_reduction = if supply > 0 { amount * aum / supply } else { 0 };

        write_balance(&e, &from, bal - amount);
        set_supply(&e, supply - amount);
        set_aum(&e, aum - aum_reduction);

        e.events()
            .publish((symbol_short!("burn"), from.clone()), (amount,));
    }

    fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        assert!(amount > 0, "amount must be positive");

        let a = get_allowance(&e, &from, &spender);
        assert!(
            a.expiration_ledger >= e.ledger().sequence(),
            "allowance expired"
        );
        assert!(a.amount >= amount, "insufficient allowance");

        write_allowance(&e, &from, &spender, a.amount - amount, a.expiration_ledger);

        let bal = balance_of(&e, &from);
        assert!(bal >= amount, "insufficient balance");

        let supply = get_supply(&e);
        let aum = get_aum(&e);
        let aum_reduction = if supply > 0 { amount * aum / supply } else { 0 };

        write_balance(&e, &from, bal - amount);
        set_supply(&e, supply - amount);
        set_aum(&e, aum - aum_reduction);

        e.events()
            .publish((symbol_short!("burn"), from.clone()), (amount,));
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
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Env,
    };

    struct Setup {
        env: Env,
        operator: Address,
        protocol: Address,
        classic_wallet: Address,
        fund_id: Address,
        usdc_id: Address,
    }

    fn setup() -> Setup {
        let e = Env::default();
        e.mock_all_auths();

        let admin = Address::generate(&e);
        let operator = Address::generate(&e);
        let protocol = Address::generate(&e);
        let classic_wallet = Address::generate(&e);

        let usdc_id = e
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let fund_id = e.register(Fund, ());

        FundClient::new(&e, &fund_id).initialize(
            &admin,
            &operator,
            &protocol,
            &usdc_id,
            &classic_wallet,
            &String::from_str(&e, "MUTAV"),
            &String::from_str(&e, "MUTAV"),
            &250u32,     // exit_cap_bps: 2.5%
            &100u32,     // mgmt_fee_bps: 1%
            &25u32,      // redemption_fee_bps: 0.25%
            &2_000u32,   // protocol_fee_bps: 20%
            &604_800u64, // fulfill_window_seconds: 7 days
            &1_000u32,   // max_aum_increase_bps: 10% per call
        );

        let _ = admin; // used only in initialize; not needed after setup
        Setup {
            env: e,
            operator,
            protocol,
            classic_wallet,
            fund_id,
            usdc_id,
        }
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
        let usdc = token::Client::new(&s.env, &s.usdc_id);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);

        assert_eq!(fund.total_supply(), 100_000_000);
        assert_eq!(fund.aum(), 100_000_000);
        assert_eq!(fund.nav(), 10_000_000); // 1.0
        assert_eq!(fund.balance(&investor), 100_000_000);
        // Deposit goes to Classic wallet — contract retains no USDC
        assert_eq!(usdc.balance(&s.classic_wallet), 100_000_000);
        assert_eq!(usdc.balance(&s.fund_id), 0);
    }

    #[test]
    fn second_deposit_is_proportional() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let alice = Address::generate(&s.env);
        let bob = Address::generate(&s.env);

        usdc_mint(&s, &alice, 100_000_000);
        fund.deposit_investor(&alice, &100_000_000);

        // 10% cap on 100M AUM = 10M max; add exactly 10M
        usdc_mint(&s, &s.fund_id, 10_000_000);
        fund.add_tenant_fee(&10_000_000);
        assert_eq!(fund.nav(), 11_000_000); // NAV = 1.1

        usdc_mint(&s, &bob, 100_000_000);
        fund.deposit_investor(&bob, &100_000_000);

        // bob gets 100M * 100M / 110M = ~90_909_090 MUTAV
        let bob_bal = fund.balance(&bob);
        assert!(bob_bal > 0);
        assert_eq!(fund.aum(), 210_000_000);
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

        // 1% of 100 USDC = 1 USDC fee → AUM decreases, supply unchanged → NAV decreases
        assert_eq!(fund.aum(), 99_000_000);
        assert_eq!(fund.total_supply(), 100_000_000);
        // Fee payment is off-chain (Classic wallet → protocol); no USDC moves through contract
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
    fn receive_payment_splits_by_protocol_fee_bps() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let imob = Address::generate(&s.env);
        let usdc = token::Client::new(&s.env, &s.usdc_id);

        // protocol_fee_bps = 2_000 (20%) set in setup()
        usdc_mint(&s, &s.operator, 100_000_000);
        fund.receive_payment(&imob, &100_000_000);

        let expected_protocol = 100_000_000i128 * fund.protocol_fee_bps() as i128 / 10_000;
        let expected_fund = 100_000_000 - expected_protocol;

        assert_eq!(usdc.balance(&s.protocol), expected_protocol); // 20M
        assert_eq!(usdc.balance(&s.classic_wallet), expected_fund); // 80M
        assert_eq!(usdc.balance(&s.fund_id), 0);
        assert_eq!(fund.aum(), expected_fund);
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

        // Yield arrives: 10% cap = 10M; add exactly 10M
        usdc_mint(&s, &s.fund_id, 10_000_000);
        fund.add_yield(&10_000_000);
        assert_eq!(fund.nav(), 11_000_000); // 1.1

        // cap = 2.5% of AUM(110M) = 2_750_000 USDC; 1M MUTAV @ NAV=1.1 = 1.1M USDC → fits ✓
        let total_needed = fund.process_redemptions();
        assert_eq!(total_needed, 1_100_000); // 1M MUTAV * 110M AUM / 100M supply
        assert_eq!(fund.ready_redemption(&investor), 1_100_000);
        assert_eq!(fund.queue_len(), 0);

        // Backend deposits USDC from off-ramp and fulfills.
        // 0.25% fee on 1_100_000 = 2_750 → investor gets 1_097_250, protocol gets 2_750.
        usdc_mint(&s, &s.fund_id, 1_100_000);
        fund.fulfill_redemption(&investor);

        let usdc = token::Client::new(&s.env, &s.usdc_id);
        assert_eq!(usdc.balance(&investor), 1_097_250);
        assert_eq!(usdc.balance(&s.protocol), 2_750);
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

        fund.request_redemption(&alice, &10_000_000); // 1 USDC → fits cap
        fund.request_redemption(&bob, &990_000_000); // 99 USDC → exceeds cap

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
    fn redemption_fee_goes_to_protocol() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);
        let usdc = token::Client::new(&s.env, &s.usdc_id);

        // Deposit 100 USDC → 100M MUTAV at NAV=1.0
        // Cap = 2.5% of 100M = 2_500_000 USDC/week
        // Request 1_000_000 MUTAV = 1 USDC → fits cap
        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &1_000_000);

        let gross = fund.process_redemptions();
        assert_eq!(gross, 1_000_000);

        // fee = 1_000_000 * 25 / 10_000 = 2_500 (0.25%)
        usdc_mint(&s, &s.fund_id, gross);
        fund.fulfill_redemption(&investor);

        assert_eq!(usdc.balance(&investor), 997_500); // gross - fee
        assert_eq!(usdc.balance(&s.protocol), 2_500); // fee
    }

    #[test]
    fn record_offchain_payout_reduces_aum() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);
        let landlord = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);

        fund.record_offchain_payout(&10_000_000, &landlord);

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

    #[test]
    fn reclaim_restores_position_after_deadline() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &1_000_000);
        fund.process_redemptions();

        assert_eq!(fund.ready_redemption(&investor), 1_000_000);
        assert_eq!(fund.balance(&investor), 99_000_000);
        assert_eq!(fund.aum(), 99_000_000); // 1M USDC removed from AUM

        // Backend never fulfills — advance past the 7-day window
        s.env.ledger().with_mut(|l| l.timestamp = 604_801);
        fund.reclaim_expired_redemption(&investor);

        // MUTAV and AUM fully restored
        assert_eq!(fund.balance(&investor), 100_000_000);
        assert_eq!(fund.aum(), 100_000_000);
        assert_eq!(fund.total_supply(), 100_000_000);
        assert_eq!(fund.ready_redemption(&investor), 0);
    }

    #[test]
    fn sweep_usdc_recovers_orphaned_usdc_after_reclaim() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);
        let usdc = token::Client::new(&s.env, &s.usdc_id);

        // Deposit 10 USDC; request 1 USDC worth of redemption
        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &1_000_000);
        let gross = fund.process_redemptions();
        assert_eq!(gross, 1_000_000);

        // Backend deposits USDC for the ready redemption
        usdc_mint(&s, &s.fund_id, gross);
        assert_eq!(usdc.balance(&s.fund_id), gross);

        // Deadline passes without fulfillment — investor reclaims
        s.env.ledger().with_mut(|l| l.timestamp = 604_801);
        fund.reclaim_expired_redemption(&investor);

        // AUM is restored; USDC is orphaned in the contract
        assert_eq!(fund.aum(), 100_000_000);
        assert_eq!(usdc.balance(&s.fund_id), gross); // still here, not yielding

        let classic_before = usdc.balance(&s.classic_wallet);

        // Operator sweeps orphaned USDC back to Classic wallet — AUM unchanged
        fund.sweep_usdc(&gross);

        assert_eq!(usdc.balance(&s.fund_id), 0);
        assert_eq!(usdc.balance(&s.classic_wallet), classic_before + gross);
        assert_eq!(fund.aum(), 100_000_000); // accounting untouched
    }

    #[test]
    fn propose_and_accept_admin() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let new_admin = Address::generate(&s.env);

        fund.propose_admin(&new_admin);
        fund.accept_admin();

        // New admin can call owner-only function
        let new_wallet = Address::generate(&s.env);
        fund.set_classic_wallet(&new_wallet); // would panic if auth not accepted
    }

    #[test]
    fn set_operator_changes_hot_wallet() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let new_op = Address::generate(&s.env);

        fund.set_operator(&new_op);
        assert_eq!(fund.operator(), new_op);

        // New operator can call operator-level function
        usdc_mint(&s, &new_op, 100_000_000);
        fund.receive_payment(&Address::generate(&s.env), &100_000_000);
    }

    #[test]
    fn burn_keeps_nav_stable() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let alice = Address::generate(&s.env);
        let bob = Address::generate(&s.env);

        // Alice and Bob each deposit 100 USDC → 100M MUTAV each; AUM = 200M, NAV = 1.0
        usdc_mint(&s, &alice, 100_000_000);
        usdc_mint(&s, &bob, 100_000_000);
        fund.deposit_investor(&alice, &100_000_000);
        fund.deposit_investor(&bob, &100_000_000);
        assert_eq!(fund.nav(), 10_000_000); // 1.0
        assert_eq!(fund.aum(), 200_000_000);

        // Alice burns her 100M MUTAV directly (outside redemption queue).
        // NAV must stay at 1.0; Bob's position is unaffected.
        fund.burn(&alice, &100_000_000);

        assert_eq!(fund.balance(&alice), 0);
        assert_eq!(fund.total_supply(), 100_000_000); // only Bob's tokens remain
        assert_eq!(fund.aum(), 100_000_000); // proportional AUM removed
        assert_eq!(fund.nav(), 10_000_000); // NAV unchanged — Bob not affected
    }

    #[test]
    fn cancel_is_o1_ghost_cleaned_by_process() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &100_000_000);

        assert_eq!(fund.queue_len(), 1);

        // Cancel: O(1) — no queue rebuild; ghost entry remains internally.
        fund.cancel_redemption(&investor);

        assert_eq!(fund.balance(&investor), 100_000_000);
        assert_eq!(fund.pending_redemption(&investor), 0);
        // queue_len filters ghosts — must report 0.
        assert_eq!(fund.queue_len(), 0);

        // Ghost is purged automatically when process_redemptions runs.
        fund.process_redemptions();
        assert_eq!(fund.queue_len(), 0);
    }

    #[test]
    fn add_yield_enforces_aum_cap() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        // AUM = 100M; 10% cap = 10M max per call

        // Exactly at cap — should pass
        usdc_mint(&s, &s.fund_id, 10_000_000);
        fund.add_yield(&10_000_000);
        assert_eq!(fund.aum(), 110_000_000);
    }

    #[test]
    #[should_panic(expected = "AUM increase exceeds per-call cap")]
    fn add_yield_rejects_when_aum_is_zero() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        // No deposits — AUM = 0, cap = 0; any yield should be rejected
        fund.add_yield(&1i128);
    }

    #[test]
    #[should_panic(expected = "AUM increase exceeds per-call cap")]
    fn add_tenant_fee_rejects_when_aum_is_zero() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        fund.add_tenant_fee(&1i128);
    }

    #[test]
    fn remove_registry_disables_approval_check() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);

        // Set a registry, then remove it
        let registry = Address::generate(&s.env);
        fund.set_registry(&registry);
        fund.remove_registry();

        // After removal, receive_payment must not invoke the registry
        // (it would panic if it tried — registry address is gone)
        usdc_mint(&s, &s.operator, 100_000_000);
        let imob = Address::generate(&s.env);
        fund.receive_payment(&imob, &100_000_000); // must not panic
    }

    #[test]
    fn process_redemptions_respects_max_queue_batch() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);

        // Fill the queue with MAX_QUEUE_BATCH + 5 = 45 investors.
        // Each deposits 1 USDC; NAV = 1.0 so each holds 10M MUTAV.
        // AUM = 45 * 10M = 450M; cap = 2.5% = 11.25M.
        // First investor (10M USDC) fits the cap; the rest are deferred.
        // The 5 investors beyond the batch ceiling are carried forward unexamined.
        let n: i128 = 45;
        let deposit_per = 10_000_000i128;
        for _ in 0..n {
            let inv = Address::generate(&s.env);
            usdc_mint(&s, &inv, deposit_per);
            fund.deposit_investor(&inv, &deposit_per);
            fund.request_redemption(&inv, &deposit_per);
        }

        fund.process_redemptions();

        // At least 5 entries carried over from beyond the batch ceiling.
        assert!(fund.queue_len() >= 5);
    }

    #[test]
    #[should_panic(expected = "AUM increase exceeds per-call cap")]
    fn add_yield_rejects_above_cap() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        // AUM = 100M; 10% cap = 10M; 10_000_001 exceeds it

        usdc_mint(&s, &s.fund_id, 10_000_001);
        fund.add_yield(&10_000_001);
    }

    #[test]
    #[should_panic(expected = "fulfill window has expired")]
    fn fulfill_rejects_after_deadline() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &1_000_000);
        fund.process_redemptions();

        // Advance past the 7-day fulfill window
        s.env.ledger().with_mut(|l| l.timestamp = 604_801);

        // Operator tries to fulfill after deadline — must panic
        usdc_mint(&s, &s.fund_id, 1_000_000);
        fund.fulfill_redemption(&investor);
    }

    #[test]
    fn extend_balance_ttl_keeps_entry_alive() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        assert_eq!(fund.balance(&investor), 100_000_000);

        // Anyone can extend the TTL without auth
        fund.extend_balance_ttl(&investor);
        assert_eq!(fund.balance(&investor), 100_000_000);
    }

    #[test]
    fn extend_redemption_ttl_is_permissionless_and_preserves_value() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &1_000_000);

        assert_eq!(fund.pending_redemption(&investor), 1_000_000);

        // Any address can extend TTL — must not panic and must preserve value
        fund.extend_redemption_ttl(&investor);
        assert_eq!(fund.pending_redemption(&investor), 1_000_000);
    }

    #[test]
    fn extend_redemption_ttl_is_noop_when_no_entry() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        // No pending or ready redemption — must not panic
        fund.extend_redemption_ttl(&investor);
    }

    #[test]
    fn large_entry_does_not_block_smaller_entries_behind_it() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let alice = Address::generate(&s.env);
        let bob = Address::generate(&s.env);

        // AUM = 1100 USDC; cap = 2.5% = 27.5 USDC
        // alice requests 100 USDC (exceeds cap); bob requests 1 USDC (fits)
        usdc_mint(&s, &alice, 1_000_000_000);
        usdc_mint(&s, &bob, 100_000_000);
        fund.deposit_investor(&alice, &1_000_000_000);
        fund.deposit_investor(&bob, &100_000_000);

        fund.request_redemption(&alice, &1_000_000_000);
        fund.request_redemption(&bob, &10_000_000);

        // alice exceeds cap → deferred; bob fits → processed despite being behind alice
        let total = fund.process_redemptions();

        assert_eq!(total, 10_000_000);                           // bob's amount only
        assert_eq!(fund.pending_redemption(&alice), 1_000_000_000); // alice deferred
        assert_eq!(fund.ready_redemption(&bob), 10_000_000);    // bob unblocked
        assert_eq!(fund.queue_len(), 1);                         // only alice remains
    }

    #[test]
    fn config_setters_update_values() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);

        fund.set_exit_cap_bps(&500u32);
        assert_eq!(fund.exit_cap_bps(), 500);

        fund.set_mgmt_fee_bps(&200u32);
        assert_eq!(fund.mgmt_fee_bps(), 200);

        fund.set_redemption_fee_bps(&50u32);
        assert_eq!(fund.redemption_fee_bps(), 50);

        fund.set_protocol_fee_bps(&1_000u32);
        assert_eq!(fund.protocol_fee_bps(), 1_000);

        fund.set_max_aum_increase_bps(&300u32);
        assert_eq!(fund.max_aum_increase_bps(), 300);

        fund.set_fulfill_window(&1_209_600u64); // 14 days
    }

    #[test]
    #[should_panic(expected = "exit_cap_bps must be 1..10_000")]
    fn set_exit_cap_bps_rejects_above_max() {
        let s = setup();
        FundClient::new(&s.env, &s.fund_id).set_exit_cap_bps(&10_001u32);
    }

    #[test]
    #[should_panic(expected = "exit_cap_bps must be 1..10_000")]
    fn set_exit_cap_bps_rejects_zero() {
        let s = setup();
        FundClient::new(&s.env, &s.fund_id).set_exit_cap_bps(&0u32);
    }

    #[test]
    fn redemption_fee_is_snapshotted_at_process_time() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);
        let usdc = token::Client::new(&s.env, &s.usdc_id);

        // Deposit and request; fee_bps = 25 at process time
        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &1_000_000);
        let gross = fund.process_redemptions();
        assert_eq!(gross, 1_000_000);

        // Admin raises fee to 10% after processing but before fulfillment
        fund.set_redemption_fee_bps(&1_000u32);

        // Backend deposits and fulfills — investor must still pay the original 0.25% fee
        usdc_mint(&s, &s.fund_id, gross);
        fund.fulfill_redemption(&investor);

        let expected_fee = gross * 25 / 10_000; // 25 bps, not 1_000
        assert_eq!(usdc.balance(&investor), gross - expected_fee);
        assert_eq!(usdc.balance(&s.protocol), expected_fee);
    }

    #[test]
    #[should_panic(expected = "mgmt_fee_bps exceeds 10%")]
    fn set_mgmt_fee_bps_rejects_above_max() {
        let s = setup();
        FundClient::new(&s.env, &s.fund_id).set_mgmt_fee_bps(&1_001u32);
    }

    #[test]
    #[should_panic(expected = "protocol_fee_bps exceeds 50%")]
    fn set_protocol_fee_bps_rejects_above_max() {
        let s = setup();
        FundClient::new(&s.env, &s.fund_id).set_protocol_fee_bps(&5_001u32);
    }

    #[test]
    #[should_panic(expected = "max_aum_increase_bps must be 1..10_000")]
    fn set_max_aum_increase_bps_rejects_zero() {
        let s = setup();
        FundClient::new(&s.env, &s.fund_id).set_max_aum_increase_bps(&0u32);
    }

    #[test]
    fn set_exit_cap_bps_takes_effect_on_next_process() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        // Deposit 100 USDC → 100M MUTAV; default cap = 2.5% = 2.5M micro-USDC
        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);

        // Request 10M MUTAV (= 1 USDC) — exceeds 2.5M cap → deferred
        fund.request_redemption(&investor, &10_000_000);
        let total = fund.process_redemptions();
        assert_eq!(total, 0);

        // Admin raises cap to 20% = 20M micro-USDC; 10M now fits
        fund.set_exit_cap_bps(&2_000u32);
        s.env.ledger().with_mut(|l| l.timestamp = WEEK_SECONDS + 1);
        let total = fund.process_redemptions();
        assert!(total > 0);
        assert_eq!(fund.pending_redemption(&investor), 0);
    }

    #[test]
    fn ready_redemption_deadline_reflects_fulfill_window() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        // Nenhum resgate pronto ainda — deve retornar 0
        assert_eq!(fund.ready_redemption_deadline(&investor), 0);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &1_000_000);

        // Timestamp inicial = 0; process_redemptions define deadline = 0 + 604_800 (7 dias)
        fund.process_redemptions();

        let deadline = fund.ready_redemption_deadline(&investor);
        assert_eq!(deadline, 604_800); // timestamp 0 + fulfill_window_seconds configurado no setup

        // Após o prazo, o investidor pode reclamar — deadline não muda, é só uma leitura
        s.env.ledger().with_mut(|l| l.timestamp = 604_801);
        assert_eq!(fund.ready_redemption_deadline(&investor), 604_800);

        fund.reclaim_expired_redemption(&investor);

        // Depois do reclaim a entrada some — volta para 0
        assert_eq!(fund.ready_redemption_deadline(&investor), 0);
    }

    #[test]
    fn cancel_redemption_purges_ghost_on_next_process() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let alice = Address::generate(&s.env);
        let bob = Address::generate(&s.env);

        usdc_mint(&s, &alice, 100_000_000);
        usdc_mint(&s, &bob, 100_000_000);
        fund.deposit_investor(&alice, &100_000_000);
        fund.deposit_investor(&bob, &100_000_000);

        fund.request_redemption(&alice, &1_000_000);
        fund.request_redemption(&bob, &1_000_000);
        assert_eq!(fund.queue_len(), 2);

        // Alice cancela — vira ghost interno, mas queue_len já desconta
        fund.cancel_redemption(&alice);
        assert_eq!(fund.queue_len(), 1);

        // process_redemptions percorre a fila inteira; o ghost da alice é descartado
        fund.process_redemptions();
        assert_eq!(fund.queue_len(), 0);
    }

    #[test]
    fn admin_view_returns_owner() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        // admin() and operator() must both be set and distinct
        let admin_addr = fund.admin();
        let op_addr = fund.operator();
        assert_ne!(admin_addr, op_addr);
    }

    #[test]
    #[should_panic(expected = "mgmt_fee_bps exceeds 10%")]
    fn initialize_rejects_mgmt_fee_above_max() {
        let e = Env::default();
        e.mock_all_auths();
        let admin = Address::generate(&e);
        let operator = Address::generate(&e);
        let protocol = Address::generate(&e);
        let usdc_id = e
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let fund_id = e.register(Fund, ());
        FundClient::new(&e, &fund_id).initialize(
            &admin,
            &operator,
            &protocol,
            &usdc_id,
            &Address::generate(&e),
            &String::from_str(&e, "MUTAV"),
            &String::from_str(&e, "MUTAV"),
            &250u32,
            &2_000u32, // > 1_000 — must panic
            &25u32,
            &2_000u32,
            &604_800u64,
            &500u32,
        );
    }

    #[test]
    #[should_panic(expected = "exit_cap_bps exceeds 100%")]
    fn initialize_rejects_exit_cap_above_max() {
        let e = Env::default();
        e.mock_all_auths();
        let admin = Address::generate(&e);
        let operator = Address::generate(&e);
        let usdc_id = e
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let fund_id = e.register(Fund, ());
        FundClient::new(&e, &fund_id).initialize(
            &admin,
            &operator,
            &Address::generate(&e),
            &usdc_id,
            &Address::generate(&e),
            &String::from_str(&e, "MUTAV"),
            &String::from_str(&e, "MUTAV"),
            &10_001u32, // > 10_000 — must panic
            &100u32,
            &25u32,
            &2_000u32,
            &604_800u64,
            &500u32,
        );
    }

    // ── stress tests ──────────────────────────────────────────────────────────

    /// Demonstrates that a queue drains progressively in stages when the exit cap is
    /// adjusted between calls. Uses 12 investors and two process_redemptions calls:
    ///   call 1 (cap = 25%): processes 3 investors — 9 deferred
    ///   call 2 (cap = 100%, next week): processes remaining 9 — queue empty
    ///
    /// Soroban write-entry budget (50) limits how many investors can be processed per
    /// call: each costs 3 writes (remove pending + write ready + TTL ready), so the
    /// practical ceiling is ~14 per call. Entries beyond that must spill to future calls.
    #[test]
    fn stress_queue_drains_in_stages() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);

        // 12 investors × 10 USDC → AUM = 120 USDC. cap = 25% = 30 USDC/week.
        let n = 12u32;
        let deposit_per = 10_000_000i128;
        for _ in 0..n {
            let inv = Address::generate(&s.env);
            usdc_mint(&s, &inv, deposit_per);
            fund.deposit_investor(&inv, &deposit_per);
            fund.request_redemption(&inv, &deposit_per);
        }
        assert_eq!(fund.queue_len(), n);

        // Call 1: 3 investors fit the 30 USDC cap (3 × 10 = 30 ≤ 30); 4th deferred.
        fund.set_exit_cap_bps(&2_500u32);
        let week1 = fund.process_redemptions();
        assert_eq!(week1, 30_000_000, "3 investors processed at 25% cap");
        assert_eq!(fund.queue_len(), 9, "9 deferred to next call");

        // Admin raises cap to 100% — remaining 9 investors can now all exit at once.
        fund.set_exit_cap_bps(&10_000u32);

        // Call 2 (week 2): cap = 100% of remaining AUM (90 USDC). All 9 fit.
        // writes = 9 × 3 + 7 overhead = 34 ≤ 50 — within Soroban limits.
        s.env.ledger().with_mut(|l| l.timestamp = WEEK_SECONDS + 1);
        let week2 = fund.process_redemptions();
        assert_eq!(week2, 90_000_000, "remaining 9 investors drained");
        assert_eq!(fund.queue_len(), 0, "queue fully empty after 2 calls");
    }

    /// At every step: sum(balance + pending_redemption for all investors) == total_supply.
    /// ready_redemption is excluded: that MUTAV was already burned from total_supply
    /// during process_redemptions (reclaim re-mints it back if needed).
    #[test]
    fn stress_mutav_conservation() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);

        let amounts: [i128; 10] = [
            100_000_000, 200_000_000,  50_000_000, 300_000_000, 150_000_000,
             80_000_000, 120_000_000, 250_000_000,  90_000_000, 170_000_000,
        ];
        let mut investors = soroban_sdk::Vec::<Address>::new(&s.env);
        for &amt in amounts.iter() {
            let inv = Address::generate(&s.env);
            usdc_mint(&s, &inv, amt);
            fund.deposit_investor(&inv, &amt);
            investors.push_back(inv);
        }

        let check = || {
            let mut total: i128 = 0;
            for i in 0..investors.len() {
                let inv = investors.get_unchecked(i);
                total += fund.balance(&inv) + fund.pending_redemption(&inv);
            }
            assert_eq!(total, fund.total_supply());
        };

        check(); // baseline: all MUTAV is in balances

        // First 4 investors request half their MUTAV → moves to pending
        for i in 0..4u32 {
            let inv = investors.get_unchecked(i);
            let half = fund.balance(&inv) / 2;
            fund.request_redemption(&inv, &half);
        }
        check(); // pending replaces balance, supply unchanged

        // Investors 0 and 1 cancel → pending returns to balance
        fund.cancel_redemption(&investors.get_unchecked(0));
        fund.cancel_redemption(&investors.get_unchecked(1));
        check(); // cancel restores balance — supply still unchanged

        // Process: investors 2 and 3 have their MUTAV burned from supply
        fund.set_exit_cap_bps(&5_000u32);
        fund.process_redemptions();
        check(); // pending removed AND supply decreased by same amount — still balanced

        // Investor 2 reclaims after deadline → MUTAV re-minted back into supply
        s.env.ledger().with_mut(|l| l.timestamp = 604_801);
        fund.reclaim_expired_redemption(&investors.get_unchecked(2));
        check(); // re-mint adds to both balance and supply — balanced again
    }

    /// Verifies that the weekly exit cap accumulates correctly across multiple
    /// process_redemptions calls and resets cleanly at the start of a new week.
    ///
    /// Setup: 10 investors × 100 USDC = 1000 USDC AUM, exit_cap = 2.5% = 25 USDC/week.
    /// Each requests 10 USDC (1% of AUM): investors 1 and 2 fit (20 ≤ 25),
    /// investor 3 is deferred (30 > 25 cap).
    #[test]
    fn stress_cap_accumulates_within_week_and_resets_next_week() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);

        let deposit_per = 100_000_000i128;
        let request_per =  10_000_000i128;
        for _ in 0..10u32 {
            let inv = Address::generate(&s.env);
            usdc_mint(&s, &inv, deposit_per);
            fund.deposit_investor(&inv, &deposit_per);
            fund.request_redemption(&inv, &request_per);
        }
        // AUM = 1000M, cap = 2.5% = 25M/week, each request = 10M

        let call1 = fund.process_redemptions();
        assert_eq!(call1, 20_000_000, "first call: 2 investors fit the weekly cap");

        // 20M already used this week; only ~4.5M remain — nobody fits the second call
        let call2 = fund.process_redemptions();
        assert_eq!(call2, 0, "second call in same week: cap exhausted");

        // New week: cap resets, 2 more investors are processed
        s.env.ledger().with_mut(|l| l.timestamp = WEEK_SECONDS + 1);
        let call3 = fund.process_redemptions();
        assert_eq!(call3, 20_000_000, "new week: cap resets, 2 more processed");
        assert_eq!(fund.queue_len(), 6, "6 investors still waiting after 2 weeks");
    }

    /// Verifica a direção correta do NAV para cada operação:
    /// - exatamente igual: request_redemption, cancel_redemption, fulfill_redemption
    ///   (não tocam AUM nem supply)
    /// - não-decrescente: deposit, process_redemptions
    ///   (truncamento inteiro pode adicionar ≤1 unidade, sempre em favor do fundo)
    /// - estritamente maior: add_yield, add_tenant_fee
    /// - estritamente menor: charge_mgmt_fee, cover_default
    #[test]
    fn stress_nav_only_changes_on_yield_and_fees() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);

        let alice = Address::generate(&s.env);
        let bob   = Address::generate(&s.env);

        // Primeiro depósito: NAV = 1.0 e não deve mudar
        let nav0 = fund.nav();
        usdc_mint(&s, &alice, 100_000_000);
        fund.deposit_investor(&alice, &100_000_000);
        assert_eq!(fund.nav(), nav0, "primeiro depósito não pode mudar o NAV");

        // add_yield: NAV sobe
        let nav1 = fund.nav();
        fund.add_yield(&(fund.aum() / 10)); // exatamente no teto de 10%
        assert!(fund.nav() > nav1, "yield deve subir o NAV");

        // Segundo depósito com NAV elevado: não-decrescente
        let nav2 = fund.nav();
        usdc_mint(&s, &bob, 100_000_000);
        fund.deposit_investor(&bob, &100_000_000);
        assert!(fund.nav() >= nav2, "depósito não pode diminuir o NAV");

        // request_redemption: só trava MUTAV, AUM/supply intocados — exato
        let nav3 = fund.nav();
        fund.request_redemption(&alice, &5_000_000);
        assert_eq!(fund.nav(), nav3, "request_redemption não pode mudar o NAV");

        // cancel_redemption: devolve MUTAV, AUM/supply intocados — exato
        let nav4 = fund.nav();
        fund.cancel_redemption(&alice);
        assert_eq!(fund.nav(), nav4, "cancel não pode mudar o NAV");

        // process_redemptions: burn proporcional, NAV não-decrescente
        fund.request_redemption(&alice, &5_000_000);
        fund.set_exit_cap_bps(&5_000u32);
        let nav5 = fund.nav();
        fund.process_redemptions();
        assert!(fund.nav() >= nav5, "process_redemptions não pode diminuir o NAV");

        // fulfill_redemption: só move USDC, AUM/supply já acertados — exato
        let nav6 = fund.nav();
        let gross = fund.ready_redemption(&alice);
        usdc_mint(&s, &s.fund_id, gross);
        fund.fulfill_redemption(&alice);
        assert_eq!(fund.nav(), nav6, "fulfill não pode mudar o NAV");

        // add_tenant_fee: AUM sobe, NAV sobe
        let nav7 = fund.nav();
        fund.add_tenant_fee(&(fund.aum() / 10));
        assert!(fund.nav() > nav7, "receita de locatário deve subir o NAV");

        // charge_mgmt_fee: AUM cai, NAV cai
        s.env.ledger().with_mut(|l| l.timestamp = 31 * 24 * 60 * 60);
        let nav8 = fund.nav();
        fund.charge_mgmt_fee();
        assert!(fund.nav() < nav8, "taxa de gestão deve diminuir o NAV");

        // cover_default: AUM cai, NAV cai
        let nav9 = fund.nav();
        fund.cover_default(&5_000_000, &Address::generate(&s.env));
        assert!(fund.nav() < nav9, "cobertura de default deve diminuir o NAV");
    }

    /// Simula 3 rounds completos de operação: depósito → pedido → processo → fulfill.
    /// Entre rounds: yield adicionado (round 2) e fee cobrada (round 3).
    /// A invariante de conservação de MUTAV é verificada após cada operação.
    #[test]
    fn stress_load_with_deposit_redeem_cycles() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        fund.set_exit_cap_bps(&5_000u32); // 50% — suficiente pra processar todos os resgates

        let alice = Address::generate(&s.env);
        let bob   = Address::generate(&s.env);
        let carol = Address::generate(&s.env);
        let dave  = Address::generate(&s.env);

        // Invariante: sum(saldo + pending) de todos os investidores == total_supply
        let check = || {
            let accounted = fund.balance(&alice)   + fund.pending_redemption(&alice)
                          + fund.balance(&bob)     + fund.pending_redemption(&bob)
                          + fund.balance(&carol)   + fund.pending_redemption(&carol)
                          + fund.balance(&dave)    + fund.pending_redemption(&dave);
            assert_eq!(accounted, fund.total_supply());
        };

        // ── Round 1: depósitos iniciais + resgate parcial ────────────────────
        usdc_mint(&s, &alice, 100_000_000);
        usdc_mint(&s, &bob,   200_000_000);
        fund.deposit_investor(&alice, &100_000_000);
        fund.deposit_investor(&bob,   &200_000_000);
        assert_eq!(fund.nav(), 10_000_000); // 1.0 — primeiros depósitos não movem o NAV
        check();

        fund.request_redemption(&alice, &50_000_000);
        check();

        let nav_r1 = fund.nav();
        let usdc1 = fund.process_redemptions();
        assert!(fund.nav() >= nav_r1);
        usdc_mint(&s, &s.fund_id, usdc1);
        fund.fulfill_redemption(&alice);
        check();

        // ── Round 2: yield → novos investidores → resgates ──────────────────
        let nav_before_yield = fund.nav();
        fund.add_yield(&(fund.aum() / 10));
        assert!(fund.nav() > nav_before_yield);

        usdc_mint(&s, &carol, 150_000_000);
        usdc_mint(&s, &dave,   75_000_000);
        fund.deposit_investor(&carol, &150_000_000);
        fund.deposit_investor(&dave,   &75_000_000);
        check();

        fund.request_redemption(&bob,   &(fund.balance(&bob)   / 2));
        fund.request_redemption(&carol, &(fund.balance(&carol) / 3));
        check();

        s.env.ledger().with_mut(|l| l.timestamp = WEEK_SECONDS + 1);
        let usdc2 = fund.process_redemptions();
        assert!(usdc2 > 0);
        usdc_mint(&s, &s.fund_id, usdc2);
        fund.fulfill_redemption(&bob);
        fund.fulfill_redemption(&carol);
        check();

        // ── Round 3: taxa de gestão → resgates finais ────────────────────────
        s.env.ledger().with_mut(|l| l.timestamp = WEEK_SECONDS + 31 * 24 * 60 * 60 + 1);
        let nav_before_fee = fund.nav();
        fund.charge_mgmt_fee();
        assert!(fund.nav() < nav_before_fee);
        check();

        fund.request_redemption(&alice, &fund.balance(&alice));
        fund.request_redemption(&dave,  &fund.balance(&dave));
        check();

        s.env.ledger().with_mut(|l| l.timestamp = 2 * WEEK_SECONDS + 31 * 24 * 60 * 60 + 1);
        let usdc3 = fund.process_redemptions();
        usdc_mint(&s, &s.fund_id, usdc3);
        fund.fulfill_redemption(&alice);
        fund.fulfill_redemption(&dave);
        check();

        // ── Estado final: bob e carol ainda têm MUTAV ────────────────────────
        let remaining = fund.balance(&bob) + fund.balance(&carol);
        assert_eq!(fund.total_supply(), remaining, "conservação final");
        assert!(fund.aum() > 0, "AUM deve continuar positivo");
        assert!(fund.nav() > 0, "NAV deve continuar positivo");
    }

    // ── emergency pause tests ─────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "contract is paused")]
    fn pause_blocks_deposit() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        fund.set_paused(&true);

        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000); // must panic
    }

    #[test]
    fn pause_does_not_block_cancel_redemption() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        // Set up a pending redemption while unpaused
        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &1_000_000);
        assert_eq!(fund.pending_redemption(&investor), 1_000_000);

        // Pause the contract
        fund.set_paused(&true);
        assert!(fund.paused());

        // Investor must still be able to cancel even while paused
        fund.cancel_redemption(&investor);
        assert_eq!(fund.pending_redemption(&investor), 0);
        assert_eq!(fund.balance(&investor), 100_000_000);
    }

    #[test]
    fn pause_does_not_block_reclaim_expired_redemption() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        // Set up a ready redemption while unpaused
        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        fund.request_redemption(&investor, &1_000_000);
        fund.process_redemptions();
        assert_eq!(fund.ready_redemption(&investor), 1_000_000);

        // Pause and advance past the fulfill deadline
        fund.set_paused(&true);
        s.env.ledger().with_mut(|l| l.timestamp = 604_801);

        // Reclaim must work even while paused — investor can always recover their position
        fund.reclaim_expired_redemption(&investor);
        assert_eq!(fund.balance(&investor), 100_000_000);
        assert_eq!(fund.ready_redemption(&investor), 0);
    }

    #[test]
    fn unpause_restores_operations() {
        let s = setup();
        let fund = FundClient::new(&s.env, &s.fund_id);
        let investor = Address::generate(&s.env);

        fund.set_paused(&true);
        assert!(fund.paused());

        fund.set_paused(&false);
        assert!(!fund.paused());

        // Deposit works normally after unpause
        usdc_mint(&s, &investor, 100_000_000);
        fund.deposit_investor(&investor, &100_000_000);
        assert_eq!(fund.balance(&investor), 100_000_000);
    }
}
