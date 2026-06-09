# Stage-1 reserve vault — design

**Date:** 2026-06-08 (last revised: 2026-06-09)
**Branch:** `docs/stage1-reserve-vault-design`
**Status:** Draft for brainstorm review. All locked decisions from the 2026-06-09 design session folded in.
**Target crate:** [`contracts/stage1/reserve_vault/`](../../contracts/stage1/reserve_vault/) (stub already on `main` per [#95](https://github.com/mutav-finance/mutav-stellar/pull/95)).

## Context

Per the whitepaper Stage-1 architecture ([`mutav/docs/mutav-whitepaper.en.md` §5.4](https://github.com/mutav-finance/mutav/blob/main/docs/mutav-whitepaper.en.md)) and the research grounding in [`mutav/research/01-Protocol/reserve-asset-and-onchain-ramp.md`](https://github.com/mutav-finance/mutav/blob/main/research/01-Protocol/reserve-asset-and-onchain-ramp.md) + [`mutav/research/03-Stellar-Soroban/pilot-architecture-on-stellar.md`](https://github.com/mutav-finance/mutav/blob/main/research/03-Stellar-Soroban/pilot-architecture-on-stellar.md), Stage 1 puts the on-chain reserve that backs every live *fiança onerosa* guarantee inside a Soroban contract. The reserve is capitalised from two flows — (a) Mutav's pre-seed raise and (b) the 80% reserve allocation that the licensed sub-adquirente splits off at settlement from each agency guarantee fee — held in a multi-asset allowlist that includes Etherfuse TESOURO (BRL-yield, ~Selic) alongside a USDC liquidity sleeve, and is publicly verifiable from chain state so that "reserve adequacy" is something any third party can compute (the structural answer to the QuintoCred unfunded-garantidora failure mode).

This spec covers **only the on-chain vault and its management**. Out of scope: the SAC-wrapped `MUTAV-COL` collateral token leg ([`contracts/stage1/collateral_token/`](../../contracts/stage1/collateral_token/) — scripts, not a Soroban contract); Etherfuse `mintBond` / `redeemBonds` mechanics (operator-driven externally); Pix ramp via licensed Stellar anchor; the 20/80 PSP-level split (off-chain at the BaaS); pre-seed off-chain custody (treasury wallet). Per the founder decision recorded in the design session, **the operator manages Etherfuse trading off-chain** — the vault is a custodial reserve holder + event ledger + admin-gated default-payout, not an automated treasury manager.

## Scope

What this contract is:

- A **custodial holder** of a Mutav-admin-managed allowlist of SEP-41 token balances. Always at least USDC + TESOURO at pilot start; admin may add other BRL-yield assets later (BRS, BRLV) without contract upgrade.
- An **event ledger** for inbound capital arrivals, operator-driven swaps between approved assets, and admin-driven default-payouts to agencies — every state transition replay-guarded by a Stellar tx-hash key and emitted as a Soroban event for off-chain reconciliation against the Mutav-app Convex backend.
- A **drain-defense surface** via `pay_default` — admin-only authority, 24h timelock, per-item max enforced at propose, batch DoS caps. No per-period rolling cap at the pilot (deliberately simplified — see §Simplifications).
- A **public verifier surface** via raw per-asset balance + rate views, so the whitepaper §5.4.3 transparency recipe is executable from chain state alone. The denomination-equivalent sum is computed *off-chain* by the consumer using the contract's exposed inputs — the contract does no rate math in views.

What this contract is **not**:

- Not a SEP-56 / SEP-41 share-issuing vault. Stage-1 capitalisation is the pre-seed + per-payment 80% reserve allocation; there is no Stage-1 investor-deposit path. Share mechanics arrive at Stage 2 (§6 / §7) under a separate contract or refactor.
- Not an automated Etherfuse caller. The operator drives `mintBond` / `redeemBonds` from outside; the contract observes balance arrivals and logs operator-supplied references.
- Not a payment splitter. The 20/80 (operational / reserve) split happens at the licensed sub-adquirente, off-chain. The vault sees only the 80% reserve portion arriving as USDC after the operator's BRL→USDC ramp.
- Not a custom-account contract. Admin is a regular Stellar keypair (Mutav HW wallet). Per the Stellar contract-accounts guide, "stay with a classic account when you want the simplest path" — Phase 1 wants the simplest path. The vault's `set_admin` mechanism leaves the door open to swap in a `DefaultManager` contract address later without changing the vault contract.
- Not a payments router. Inbound USDC arrives via plain SEP-41 transfers (operator initiates externally); the vault logs the arrival via `record_capital_receipt`.

This separation matches whitepaper §5.5's authority table: the audited surface holds no keys; operator authority is custodied off-repo per [`docs/specs/2026-05-31-operator-key-runbook-design.md`](./2026-05-31-operator-key-runbook-design.md); admin authority lives in a hardware-wallet flow on `mutav-app/apps/admin/`.

## Trust model

| Authority | Who | What they can do here |
|---|---|---|
| **Admin** (governance multisig) | OpenZeppelin Smart Account contract instance, 3-of-5 passkey threshold (Mutav admin team members each register a passkey); managed via `mutav-app/apps/admin/` dashboard with Smart Account Kit SDK | `initialize`, `set_*` (all knobs), allowlist `add` / `remove` (both assets and outbound destinations), `set_denomination_asset`, propose / execute / cancel `pay_default`, `propose_admin` / `accept_admin`, `set_paused`, `cancel_pending_swap` |
| **Operator** (KMS-backed) | Stellar keypair held in Convex Action KMS on `mutav-app`; invoked with short-lived OIDC credentials per per-action policy. **Outbound destinations are constrained on-chain to an admin-managed allowlist** — operator cannot transfer value out of the vault to any other Address. | `record_capital_receipt`, `operator_outbound` (destination in allowlist only), `record_swap_in`, `set_asset_rate`, `publish_snapshot`, `extend_ttl` |
| **Public verifier** | Anyone with a Stellar RPC | All views; permissionless TTL extension on audit-anchor entries |
| **Pre-seed wallet / reserve-allocation wallet** | External Mutav-controlled accounts | Send USDC into the contract via plain SEP-41 transfer; the operator then calls `record_capital_receipt` to log the arrival with a tx-hash replay guard |

Threat model assumptions worth naming:

- **Operator compromise** is bounded by the **on-chain destination allowlist** — operator can only outbound to addresses admin has previously whitelisted (typically: a Mutav-controlled operator wallet for ramping into Etherfuse, possibly an emergency reseed wallet). Operator cannot transfer to an attacker-controlled wallet because that address would not be in the allowlist. Defense layering: (i) on-chain destination allowlist (primary), (ii) snapshot publication makes any whitelisted movement visible in seconds, (iii) admin `set_paused(true)` halts outbound entirely, (iv) KMS-side per-action policy on the operator key, (v) operator cannot call `pay_default` — that path is admin-only.
- **Admin compromise** can drain via `pay_default` up to `max_items_per_batch × pay_default_max_item_value` per 24h timelock window — bounded by per-batch and per-item ceilings but not by a rolling-period cap at the pilot. The 24h timelock is the live-detection window; `set_paused(true)` is the kill-switch. See §Simplifications for the explicit tradeoff.
- **Joint compromise** is not defended against on-chain. The off-chain key-custody separation is the protection.

## Verified Stellar primitives (2026-06-09 verification pass)

Cross-checked against developers.stellar.org and primary spec sources before locking the design:

- **Storage tiers** ([state-archival docs](https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival)) — three tiers with confirmed semantics:
  - **Temporary**: permanently deleted at TTL=0; cannot be restored. Right for short-lived replay guards (operator-tx-hash dedup).
  - **Persistent**: archived at TTL=0; restorable via `InvokeHostFunction` (current ledger + 4095 ledgers of life upon restore). Right for audit-trail anchors (pending swap records, pending pay_default proposals, per-asset rate attestations).
  - **Instance**: archived at TTL=0; restorable. Right for config + governance state (admin, operator, allowlist, denomination, all `set_*` knobs).
  - TTL is in *ledgers*, not seconds. `extend_ttl(threshold, extend_to)` is the Soroban SDK API (two-argument form per the SDK, even though docs simplify to single-parameter).
- **SAC (CAP-46-6)** ([SAC docs](https://developers.stellar.org/docs/tokens/stellar-asset-contract)) — classic Stellar assets wrapped by the SAC expose the SEP-41 token interface. `token::Client::balance(&env, &asset_addr).balance(self)` returns the SEP-41 balance. AUTH_REQUIRED / AUTH_REVOCABLE / AUTH_CLAWBACK_ENABLED flags transfer from the classic-asset side. The vault reads SAC-wrapped assets identically to native Soroban SEP-41 tokens — no special handling needed.
- **Contract authorization** ([auth docs](https://developers.stellar.org/docs/build/guides/auth/contract-authorization)) — `require_auth` on the admin Address dispatches correctly whether the admin is a regular keypair or a contract account implementing `CustomAccountInterface`. Phase 1 uses the latter for admin (OZ Smart Account, see below) and a regular Stellar keypair for operator.
- **CustomAccountInterface signature** ([check-auth tutorials](https://developers.stellar.org/docs/build/guides/auth/check-auth-tutorials)) — `pub fn __check_auth(env: Env, signature_payload: BytesN<32>, signature: BytesN<64>, _auth_context: Vec<Context>)`. Returns `()`, panics on failure. Implemented inside the OZ Smart Account that holds Mutav admin authority.
- **SEP-56 tokenized vault standard** ([sep-0056.md](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0056.md)) — Draft v0.1.2 (last updated 2025-11-06), OZ + Sentinel co-authored. 17-function interface (deposit/redeem/preview/conversion/maxes). **Not used by Phase 1** — the vault has no share token. Reference for Stage-2 rebuild.
- **OpenZeppelin Stellar Contracts v0.7.2** ([repo](https://github.com/OpenZeppelin/stellar-contracts)) — explicitly labelled "experimental software" by OZ themselves. Has `audits/` directory but no audit reports cited in README.
  - **Not used as runtime dependency in the vault contract itself** — we hand-roll the small primitive surface we need (admin/operator auth, paused, two-step rotation) from raw Soroban SDK. Their virtual-decimals-offset formula is referenced for the eventual Stage-2 share-vault refactor.
  - **Used as the admin smart-wallet** — the vault's `admin: Address` at deploy is an OpenZeppelin Smart Account contract instance ([`stellar-accounts` crate](https://github.com/OpenZeppelin/stellar-contracts)), configured as 3-of-5 passkey multisig via the [Smart Account Kit SDK](https://github.com/kalepail/smart-account-kit). Mutav admin team members each register one passkey; any 3 must sign to authorize a vault tx. Authority resolution is handled entirely by Soroban's `require_auth` flow — the vault doesn't know or care that admin is a contract account vs. a keypair. Audit responsibility: a focused secondary audit on the pinned OZ Smart Account version is included in Mutav's Phase 1 audit scope.

These verified primitives are the audit baseline. Reviewer can read each section of this spec against the referenced canonical doc and confirm the design uses the primitive as intended.

## Whitepaper assumptions this design depends on

Each item below is an explicit reading of §5 + the research vault that the contract surface bakes in. If any of these later turns out to be wrong, the contract surface needs to change.

1. **Reserve composition** (§5.4.1, [`reserve-asset-and-onchain-ramp.md`](https://github.com/mutav-finance/mutav/blob/main/research/01-Protocol/reserve-asset-and-onchain-ramp.md)) — **Etherfuse TESOURO** as the yield asset + **USDC-on-Stellar** as the liquidity sleeve, at pilot. Both are SEP-41-compliant tokens with a known contract address per environment. The admin-managed allowlist accommodates additional assets (BRS, BRLV) once they ship on Stellar — no redeploy needed.
2. **TESOURO is held as an SEP-41 balance** ([`etherfuse-stablebonds.md`](https://github.com/mutav-finance/mutav/blob/main/research/06-Sources/etherfuse-stablebonds.md)). Yield accrues *in the token price*, not via rebase. So a fixed TESOURO balance grows in BRL-denominated value over time; the USDC-denominated value depends on the TESOURO↔USDC reference rate. Yield disposition is "compounds in reserve" — no distribution.
3. **Operator drives Etherfuse from outside** (founder decision 2026-06-08). The contract never calls `mintBond` / `redeemBonds`. It records before-state and after-state via paired operator entry points: a subscription is one `operator_outbound(YieldAssetSubscription, ...)` + one `record_swap_in(TESOURO, ...)`; a redemption is the inverse.
4. **The 80/20 split is off-chain at the PSP** (whitepaper §5.3 architectural preference; *not* a regulatory necessity since the guarantee fee is Mutav's own money per §5.3). The vault sees only the 80% reserve portion arriving — never the full charge. No on-chain splitting.
5. **One contract instance = one reserve** (§5.4.1). Multiple parallel funds (per-tenant-class, per-region) are a Stage-2/3 concern. No multi-vault accounting here.
6. **Per-asset rates are operator-attested, not on-chain-derived** (§5.4.3). Stage 1 has no on-chain oracle for Selic or for TESOURO. The contract stores the operator's last-attested rate per asset with a `set_at` timestamp and a `valid_until` window. Rate math happens *off-chain* in the transparency portal — the contract exposes raw per-asset balances + raw per-asset rate attestations and lets the consumer multiply. The denomination asset (USDC at pilot) has implicit rate 1:1 and no rate storage.
7. **Replay protection** uses a mix of temporary and persistent storage:
   - **Operator tx-hashes** (capital receipts, outbounds, swap-ins): `SeenTxHash(BytesN<32>)` in temporary storage with ~7-day TTL — bounded operator-runtime cursor lag protection.
   - **PendingSwap records**: persistent storage keyed by `op_tx_hash` — tracks in-flight value during operator-driven swap-out/swap-in cycles so transparency portal can show "in operation" alongside "custodied."
   - **PendingPayDefaultProposal records**: persistent storage keyed by monotonic `proposal_id` — the multi-pending queue.
8. **`pay_default` is the only on-chain debit path** for reserve outflow to agencies. All other outbound flows are operator-driven swap pairs with an expected `record_swap_in` companion. Yield never distributes; reserve never reverse-withdraws to ops.
9. **The whitepaper's published over-collateralisation ratio K** (§11c, §5.4.3) is *not enforced on-chain*. The contract does not know live book exposure — that is read off-chain from the Convex-side guarantee registry and from the `MUTAV-COL` issuer account supply (§5.4.3 step (c)). The vault publishes its side (per-asset balances + per-asset rates + per-asset pending-swap values); the verifier divides. Stage 1 chooses transparency over enforcement.
10. **The Mutav-side architecture has two accounts**: an **operational account** receiving the 20% PSP-split portion as company cashflow (off-vault, opaque, no transparency obligation), and **this reserve vault** receiving the 80% — the verifiable single-purpose collateral backing the guarantees. Vault never outflows to the operational account.

## Whitepaper gaps remaining (counsel / ops inputs needed)

| Gap | What's open | Blocks |
|---|---|---|
| **G1 — TESOURO Stellar asset code/issuer** | [`reserve-asset-and-onchain-ramp.md`](https://github.com/mutav-finance/mutav/blob/main/research/01-Protocol/reserve-asset-and-onchain-ramp.md) flags this as "confirm with Etherfuse." | Mainnet `initialize` call (admin must supply the right addresses). Not a spec blocker. |
| **G3 — initial `pay_default_max_item_value` calibration** | Working number is $15k USDC = R$82.5k at 5.5 BRL/USD = 1.65× the demo's R$50k highest. For mainnet, derive from actual pilot rent distribution + SEP-38 reference rate at deploy. | Init param at deploy. Admin can adjust via `set_pay_default_max_item_value` later. |
| **G4 — `guarantee_contract_hash` justification provenance off-chain** | What does the off-chain artefact look like? Per Mutav admin process: signed agreement document (canonical PDF / JSON) hashed at content level. Convex stores `content_hash → doc → guarantee details` mapping. | Operations runbook, not contract code. |
| **G5 — Pre-seed wallet custody and reserve-allocation wallet custody** | Both are Mutav-controlled external Stellar accounts. Exact custody location (HW wallet? Convex KMS? Treasury multisig?) is admin-team decision. | Operations runbook. |
| **G6 — USDC-sleeve sizing** | §5.4.4 names "peak-week payout rate × safety factor" as open. Not a contract constraint. | Operator-side trading playbook. |

All other earlier gaps (G2 rate source, G7 snapshot signature, G8 cadence enforcement, G9 outbound cap, G10 multi-asset evolution) are **closed**:
- **G2**: per-asset operator-attested rate model accepted; rate math off-chain.
- **G7**: operator-call simple version locked (admin-signed-payload deferred to Phase 2).
- **G8**: observability-only (`max_snapshot_gap_secs = 0` semantically, the field exists for future Phase 2 enforcement).
- **G9**: no on-chain operator-outbound cap at pilot; KMS-side policy is the gate.
- **G10**: multi-asset evolution built-in via the admin-managed allowlist.

## Goals

1. **The whitepaper §5.4.3 transparency recipe is executable from chain state alone.** A third party with a Stellar RPC + this contract's address + the Convex-side guarantee registry can compute reserve adequacy K with no off-chain trust assumption beyond the operator-attested per-asset rates (which they can sanity-check against independent oracles).
2. **`pay_default` is admin-gated, batched, timelocked, per-item-bounded** so a compromised admin key cannot drain the reserve in a single tx and admin operators can settle ~30 monthly defaults across ~50-item batches with bounded HW-wallet bandwidth.
3. **Every state transition is replay-guarded and event-emitting** so the on-chain log + off-chain accounting can be reconciled deterministically.
4. **The audit surface is small.** No share-token / NAV / queue logic. The contract holds balances, gates writes, and emits events. No OpenZeppelin runtime dependency (their crate is explicitly experimental). Target: under 800 lines of Rust including tests setup, well under 500 in the contract itself.
5. **Operator authority is bounded by an admin-managed on-chain destination allowlist** — even a fully compromised operator key cannot move value to an address admin hasn't pre-approved. This is the primary on-chain defense; KMS-side policy is defense-in-depth.
6. **Multi-asset reserve evolution is admin-tunable**, no redeploy. The denomination asset is mutable (depeg response). The allowlist is mutable (new BRL-yield assets).

## Non-goals

1. **No share token, no NAV math.** Stage 1 has no investors. The §7.2 wrapper-as-actor evolution path arrives later; this contract is not it.
2. **No automated Etherfuse interaction.** Per the operator-managed-trading scoping. If we later want the contract to call `mintBond` directly, that's a separate spec.
3. **No on-chain rate oracle.** Per-asset rates are operator-attested; Stage 1 doesn't have a price-feed dependency.
4. **No on-chain `total_assets()` computed sum.** Raw per-asset views only; consumer does the rate math off-chain.
5. **No on-chain 20/80 payment split.** Split happens at the licensed sub-adquirente, off-chain.
6. **No per-period `pay_default` cap at the pilot.** Per-item max + per-batch item cap + queue cap + 24h timelock + admin pause are the drain-defense stack. Per-period rolling cap is a Phase 2 addition if needed.
7. **No on-chain replay-guard at the per-(guarantee, month) granularity.** Mutav-app's pay_default UX + Convex reconciliation are the gates against accidental double-pay. Phase 2 may add this back if scale demands.
8. **No reverse-withdrawal path from reserve to operational.** Single-purpose collateral; value enters or leaves via `pay_default` only.
9. **No yield distribution.** Yield in TESOURO token price compounds in the vault.
10. **No on-chain guarantee registry.** Book exposure stays in Convex.
11. **No `CustomAccountInterface` inside the vault contract itself.** The vault calls `require_auth(admin)` agnostically. At Phase 1 deploy, admin is an OpenZeppelin Smart Account contract instance (3-of-5 passkey multisig) — passkey UX in the dashboard, multisig protection on dangerous actions, no vault contract complexity. See §Verified Stellar primitives for OZ dependency details.

## Storage shape

Instance storage (always loaded; cheap):

```rust
enum DataKey {
    // governance
    Admin,                              // Address
    Operator,                           // Address
    PendingAdmin,                       // Address (two-step handover)
    Paused,                             // bool

    // asset model
    ApprovedAssets,                     // Vec<Address>; size ≤ 8
    DenominationAsset,                  // Address; must be in ApprovedAssets

    // destination allowlist (operator_outbound destination must be in this set)
    AllowedOutboundDestinations,        // Vec<Address>; size ≤ 16

    // pay_default config (all admin-settable)
    PayDefaultMaxItemValue,             // i128, denomination stroops
    PayDefaultTimelockSecs,             // u64
    MaxItemsPerBatch,                   // u32
    MaxPendingProposals,                // u32

    // rate config
    MaxRateStalenessSecs,               // u64

    // pending pay_default queue
    NextProposalId,                     // u64
    PendingProposalsCount,              // u32 (cheap counter for max_pending_proposals check)
}
```

Persistent storage (per-key TTL-extended; audit anchors):

```rust
enum DataKey {
    AssetRate(Address),                 // RateRecord { rate, set_at, valid_until, source_proof_hash }
    PendingSwap(BytesN<32>),            // PendingSwapRecord { asset_out, amount_out, initiated_at, kind } keyed by op_tx_hash
    PendingPayDefault(u64),             // PayDefaultProposalRecord { items, propose_ts, executable_after_ts }
}
```

Temporary storage (cheaper; non-restorable; bounded TTL):

```rust
enum DataKey {
    SeenTxHash(BytesN<32>),             // 7-day rolling replay guard for operator outbound hashes
}
```

Supporting types:

```rust
#[contracttype]
struct PayDefaultItem {
    asset: Address,
    amount: i128,
    destination: Address,               // agency Stellar wallet
    guarantee_contract_hash: BytesN<32>,
    covered_month: u32,                 // YYYYMM
}

#[contracttype]
struct RateRecord {
    rate: i128,                         // stroop-precision per-denomination rate
    set_at: u64,
    valid_until: u64,
    source_proof_hash: BytesN<32>,
}

#[contracttype]
struct PendingSwapRecord {
    asset_out: Address,
    amount_out: i128,
    initiated_at: u64,
    kind: OutboundKind,                 // YieldAssetSubscription | YieldAssetRedemption
}

#[contracttype]
struct PayDefaultProposalRecord {
    items: Vec<PayDefaultItem>,
    propose_ts: u64,
    executable_after_ts: u64,
}

#[contracttype]
enum OutboundKind {
    YieldAssetSubscription,
    YieldAssetRedemption,
}
```

Storage tier rationale follows the verified Stellar primitive guidance (see §Verified Stellar primitives): instance for governance + currently-rolling counters; persistent for long-lived audit anchors that must outlast quiet operator periods; temporary for hot 7-day-rolling replay guards.

## Entry points

### Admin governance

```rust
fn initialize(
    e: Env,
    admin: Address,                                          // OZ Smart Account contract address (3-of-5 passkey multisig)
    operator: Address,                                       // Stellar keypair backed by Convex Action KMS
    initial_approved_assets: Vec<Address>,                   // must contain denomination_asset; size ≤ 8
    denomination_asset: Address,
    initial_allowed_outbound_destinations: Vec<Address>,     // Mutav operator wallet(s) + Etherfuse-routing addresses; size ≤ 16
    pay_default_max_item_value: i128,
    pay_default_timelock_secs: u64,
    max_items_per_batch: u32,
    max_pending_proposals: u32,
    max_rate_staleness_secs: u64,
);

fn set_operator(e: Env, new_operator: Address);
fn set_paused(e: Env, paused: bool);
fn propose_admin(e: Env, new_admin: Address);
fn accept_admin(e: Env);                       // called by pending admin

// asset allowlist
fn add_approved_asset(e: Env, asset: Address);                              // ≤8 total
fn remove_approved_asset(e: Env, asset: Address);                           // balance must be 0; cannot remove denomination
fn set_denomination_asset(e: Env, new_denomination: Address);               // must be in allowlist; wipes rate table (depeg response)

// outbound destination allowlist
fn add_allowed_outbound_destination(e: Env, addr: Address);                 // ≤16 total
fn remove_allowed_outbound_destination(e: Env, addr: Address);              // no balance check; just removes

// pay_default config
fn set_pay_default_max_item_value(e: Env, value: i128);                     // > 0
fn set_pay_default_timelock_secs(e: Env, secs: u64);                        // > 0
fn set_max_items_per_batch(e: Env, value: u32);                             // 1..=200
fn set_max_pending_proposals(e: Env, value: u32);                           // 1..=1000

// rate config
fn set_max_rate_staleness_secs(e: Env, secs: u64);                          // > 0

// pay_default lifecycle
fn propose_pay_default(e: Env, items: Vec<PayDefaultItem>) -> u64;
fn execute_pay_default(e: Env, proposal_id: u64);
fn cancel_pay_default(e: Env, proposal_id: u64);

// housekeeping
fn cancel_pending_swap(e: Env, op_tx_hash: BytesN<32>, justification_hash: BytesN<32>);
// sweep_unknown_token deferred to Phase 2 — Phase 1 vault doesn't need this surface; admin can add via contract upgrade if needed
```

### Operator capital + swap routing

```rust
/// Operator records that USDC (or other approved asset) arrived at the contract address via
/// a plain SEP-41 transfer from a Mutav-controlled wallet (pre-seed wallet, reserve-allocation
/// wallet, etc.). Replay-guarded by src_tx_hash. No balance change — the value already arrived.
/// Emits `capital_in(source, asset, amount, src_tx_hash)`.
fn record_capital_receipt(
    e: Env,
    source: Address,
    asset: Address,
    amount: i128,
    src_tx_hash: BytesN<32>,
);

/// Operator transfers an approved asset out of the contract to fund an off-chain Etherfuse
/// subscription or redemption. Replay-guarded. **Destination must be in the admin-managed
/// outbound allowlist** (typically: Mutav operator wallet for Etherfuse routing). Writes a
/// PendingSwap entry so the transparency portal can include in-flight value.
/// Emits `outbound(kind, asset, amount, dest, op_tx_hash)`.
fn operator_outbound(
    e: Env,
    kind: OutboundKind,                 // YieldAssetSubscription | YieldAssetRedemption
    asset: Address,                     // must be in approved_assets
    amount: i128,
    destination: Address,               // must be in allowed_outbound_destinations
    op_tx_hash: BytesN<32>,
);

/// Operator records that an approved asset arrived at the contract via an Etherfuse swap.
/// Pairs with a prior `operator_outbound`; clears the PendingSwap entry. Slippage is auditable
/// from the (paired_outbound.amount_out, this amount_in) tuple in the event.
/// Emits `swap_in(asset_in, amount_in, paired_outbound_ref, in_tx_hash)`.
fn record_swap_in(
    e: Env,
    asset_in: Address,
    amount_in: i128,
    paired_outbound_ref: BytesN<32>,
    in_tx_hash: BytesN<32>,
);
```

### Operator rate + snapshot publication

```rust
/// Operator updates the per-asset rate-to-denomination attestation. Denomination asset's rate
/// is implicit 1:1 and not settable. Emits `rate_set(asset, rate, valid_until, source_proof_hash)`.
fn set_asset_rate(
    e: Env,
    asset: Address,                     // must be in approved_assets and != denomination_asset
    rate: i128,
    source_proof_hash: BytesN<32>,
    valid_until: u64,
);

/// Publish a snapshot. Reads each approved asset's balance + pending-swap-value + rate
/// attestation, emits a structured event for the transparency portal. Operator-authenticated;
/// no admin signature payload (Phase 1 simple version).
fn publish_snapshot(e: Env);
```

### TTL maintenance (permissionless where it serves audit longevity)

```rust
fn extend_ttl(e: Env);                                      // operator: instance storage bump
fn extend_pay_default_proposal_ttl(e: Env, proposal_id: u64);  // permissionless
fn extend_pending_swap_ttl(e: Env, op_tx_hash: BytesN<32>);    // permissionless
```

### Views (all read-only; no auth)

```rust
fn admin(e: Env) -> Address;
fn operator(e: Env) -> Address;
fn pending_admin(e: Env) -> Option<Address>;
fn paused(e: Env) -> bool;

fn approved_assets(e: Env) -> Vec<Address>;
fn denomination_asset(e: Env) -> Address;
fn is_approved_asset(e: Env, asset: Address) -> bool;

fn allowed_outbound_destinations(e: Env) -> Vec<Address>;
fn is_outbound_destination_allowed(e: Env, addr: Address) -> bool;

// per-asset (raw chain truth; consumer does rate math)
fn balance(e: Env, asset: Address) -> i128;                          // reads SEP-41 token::balance(self)
fn pending_swap_value(e: Env, asset: Address) -> i128;               // sum of PendingSwap entries with asset_out == asset
fn total_balance(e: Env, asset: Address) -> i128;                    // balance + pending_swap_value

fn asset_rate(e: Env, asset: Address) -> Option<RateRecord>;         // None for denomination asset
fn asset_rate_is_stale(e: Env, asset: Address, now: u64) -> bool;    // helper

// pay_default
fn pay_default_max_item_value(e: Env) -> i128;
fn pay_default_timelock_secs(e: Env) -> u64;
fn max_items_per_batch(e: Env) -> u32;
fn max_pending_proposals(e: Env) -> u32;
fn pending_proposals_count(e: Env) -> u32;
fn get_pay_default_proposal(e: Env, proposal_id: u64) -> Option<PayDefaultProposalRecord>;

// rate
fn max_rate_staleness_secs(e: Env) -> u64;
```

The contract intentionally does **not** expose a `total_assets()` view that multiplies balances by rates. Consumers (transparency portal, SDK readers) compute denomination-equivalent totals off-chain from `balance + pending_swap_value + rate` per asset, applying whatever staleness/haircut policy they prefer.

## Events

Each state-changing entry point emits exactly one Soroban event. Topics use `symbol_short!` consistent with `stage2/fund` conventions:

| Symbol | Data tuple | Emitted by |
|---|---|---|
| `capital_in` | `(source, asset, amount, src_tx_hash)` | `record_capital_receipt` |
| `outbound` | `(kind, asset, amount, destination, op_tx_hash)` | `operator_outbound` |
| `swap_in` | `(asset_in, amount_in, paired_outbound_ref, in_tx_hash)` | `record_swap_in` |
| `rate_set` | `(asset, rate, valid_until, source_proof_hash)` | `set_asset_rate` |
| `snapshot` | `(per_asset: Vec<(asset, balance, pending_outbound, rate, set_at)>, published_at)` | `publish_snapshot` |
| `pay_prop` | `(id, item_count, executable_after_ts, items: Vec<(asset, amount, dest, guarantee_hash, covered_month)>)` | `propose_pay_default` |
| `pay_exec` | `(id, item_count, items: Vec<(asset, amount, dest, guarantee_hash, covered_month)>)` | `execute_pay_default` |
| `pay_cncl` | `(id,)` | `cancel_pay_default` |
| `asset_add` / `asset_rm` | `(asset,)` | asset allowlist management |
| `dest_add` / `dest_rm` | `(addr,)` | outbound destination allowlist management |
| `denom_set` | `(old, new)` | `set_denomination_asset` |
| `swap_cncl` | `(op_tx_hash, justification_hash)` | `cancel_pending_swap` |
| `set_paus` / `set_op` / `prop_adm` / `acc_adm` / `set_*` (config setters) | standard `(value,)` | each governance setter |

The `pay_prop` and `pay_exec` events carry **per-item detail** (asset, amount, destination agency, guarantee hash, covered month) so the off-chain indexer can reconstruct per-guarantee, per-month payment history without calling any view. This is the audit-trail anchor for the §5.4.3 transparency story.

## Invariants (must always hold)

1. `balance(asset) >= 0` for any approved asset — trivially true since SEP-41 balances are non-negative.
2. `denomination_asset ∈ approved_assets` — enforced at `initialize` and at every `add` / `remove` / `set_denomination_asset` mutation.
3. `pay_default` is the **only entry point that decrements the reserve to a non-Mutav-controlled address**. `operator_outbound` decrements but its `destination` must be in `AllowedOutboundDestinations` (typically Mutav-controlled operator wallet) and pairs with `record_swap_in` so reserve stays in vault custody via the swap result; `cancel_pending_swap` does not transfer.
4. Every `PayDefaultItem` at propose time satisfies: `item.asset ∈ approved_assets` AND `item.amount * rate(item.asset) <= pay_default_max_item_value`. For denomination asset, rate is implicit 1:1.
5. `execute_pay_default` requires `now >= proposal.executable_after_ts`. No bypass.
6. `paused == true` blocks: `record_capital_receipt`, `operator_outbound`, `record_swap_in`, `set_asset_rate`, `propose_pay_default`, `execute_pay_default`. Does **not** block: `cancel_pay_default`, `cancel_pending_swap`, `publish_snapshot`, views, TTL extensions. (Pause is "halt new motion," not "freeze observability or recovery.")
7. Operator-tx-hash replay guards: every `op_tx_hash` / `src_tx_hash` / `in_tx_hash` parameter is checked against `SeenTxHash` before write, then written. Duplicate calls panic.
8. `PendingSwap(op_tx_hash)` is written iff `operator_outbound.kind ∈ {YieldAssetSubscription, YieldAssetRedemption}`; cleared by paired `record_swap_in(paired_outbound_ref == op_tx_hash)` or by admin `cancel_pending_swap`.
9. `PendingProposalsCount <= max_pending_proposals` at all times; checked at `propose_pay_default` and decremented at `execute_pay_default` / `cancel_pay_default`.
10. Rate staleness: `asset_rate_is_stale(asset, now) ⇔ now - asset_rate.set_at > max_rate_staleness_secs`. Stale rates are flagged but not blocked from `pay_default` propose — the per-item rate check uses the last-known rate. (Trade-off: predictable propose behavior; admin discipline + 24h timelock catches anomalies.)

## Tests we want to write (TDD anchor, in order)

The spec is correct only if these tests pass. Failing test first, then code.

**Initialization + invariants:**
- `initialize_rejects_denomination_not_in_approved_assets`
- `initialize_rejects_zero_pay_default_max_item_value`
- `initialize_rejects_zero_timelock`
- `initialize_rejects_zero_max_items_per_batch`
- `initialize_rejects_zero_max_pending_proposals`
- `initialize_sets_all_storage_correctly`

**Capital receipt + replay guards:**
- `record_capital_receipt_emits_event_and_is_idempotent_on_replay`
- `record_capital_receipt_rejects_unapproved_asset`
- `record_capital_receipt_blocked_when_paused`

**Asset allowlist management:**
- `add_approved_asset_rejects_over_capacity` (>8)
- `add_approved_asset_rejects_duplicate`
- `remove_approved_asset_rejects_when_balance_nonzero`
- `remove_approved_asset_rejects_denomination`
- `set_denomination_asset_rejects_unapproved`
- `set_denomination_asset_wipes_rate_table`
- `set_denomination_asset_emits_denom_set_event`

**Outbound destination allowlist management:**
- `add_allowed_outbound_destination_rejects_over_capacity` (>16)
- `add_allowed_outbound_destination_rejects_duplicate`
- `remove_allowed_outbound_destination_removes_entry`
- `operator_outbound_rejects_non_whitelisted_destination`
- `operator_outbound_accepts_whitelisted_destination`

**Operator outbound + swap-in pairing:**
- `operator_outbound_transfers_token_and_writes_pending_swap`
- `operator_outbound_blocked_when_paused`
- `record_swap_in_clears_pending_swap`
- `record_swap_in_panics_when_no_paired_outbound`
- `cancel_pending_swap_admin_only_clears_entry`

**Rate publishing:**
- `set_asset_rate_records_validity_window`
- `set_asset_rate_rejects_denomination_asset`
- `asset_rate_is_stale_flips_after_max_staleness_secs`

**Pay_default — propose:**
- `propose_pay_default_rejects_empty_batch`
- `propose_pay_default_rejects_over_max_items_per_batch`
- `propose_pay_default_rejects_when_pending_count_at_max`
- `propose_pay_default_rejects_item_amount_above_max_item_value` (denomination asset)
- `propose_pay_default_rejects_non_denom_item_when_rate_stale`
- `propose_pay_default_rejects_item_with_unapproved_asset`
- `propose_pay_default_returns_id_and_emits_event_with_items`
- `propose_pay_default_non_admin_panics`

**Pay_default — execute:**
- `execute_pay_default_rejects_before_timelock_expires`
- `execute_pay_default_transfers_each_item_to_destination`
- `execute_pay_default_clears_pending_proposal`
- `execute_pay_default_non_admin_panics`

**Pay_default — cancel:**
- `cancel_pay_default_removes_pending_and_emits_event`
- `cancel_pay_default_non_admin_panics`

**Pay_default — batch behavior:**
- `propose_then_execute_50_item_batch_pays_all_destinations`
- `propose_pay_default_with_mixed_assets_checks_each_rate_freshness`

**Multi-month coverage:**
- `same_guarantee_can_be_paid_for_multiple_months_in_separate_proposals`
- `events_carry_per_item_guarantee_hash_and_covered_month`

**Two-step admin transfer:**
- `propose_admin_accept_admin_completes_handover`
- `propose_admin_blocks_when_pending_already_set`
- `accept_admin_panics_when_caller_is_not_pending`

**Pause:**
- `set_paused_blocks_inbound_outbound_and_pay_default`
- `set_paused_does_not_block_cancel_paths_or_views`

**Views:**
- `total_balance_equals_balance_plus_pending_swap_value`
- `views_return_correct_values_after_full_lifecycle`

## Open questions (Phase 1 shippable as-is; these inform Phase 2)

- **G3** initial `pay_default_max_item_value` calibration for mainnet (SEP-38 quote derivation).
- **G4** `guarantee_contract_hash` canonical serialization (canonical PDF? canonical JSON? — for content-hash determinism).
- **G5** pre-seed wallet + reserve-allocation wallet custody decisions (operations runbook).
- **G6** USDC-sleeve sizing target (operator trading playbook).

Each is operational, not a contract design blocker. Spec ships.

## Phasing

| PR | Work | Branch | Status |
|---|---|---|---|
| **PR-A** | This spec | `docs/stage1-reserve-vault-design` | this PR |
| **PR-B** | Verified Stellar primitives — research vault updates | `mutav` repo, new branch | edits staged, not committed |
| **PR-C** | Stage-2 `cover_default` → `pay_default` rename | separate `mutav-stellar` PR | queued |
| **PR-D** | Stage-1 `reserve_vault` TDD implementation | follow-up after PR-A merges | queued |
| **PR-E** | Stage-1 SDK additions (`src/providers/soroban/reserve_vault.ts`) | follow-up after PR-D | queued |
| **PR-F** | Mutav admin UI surface for vault knobs (mutav-app side) | mutav-app repo | out of mutav-stellar scope |

Audit gating: this contract joins the same audit cohort as `stage2/fund`. Defer mainnet deploy until audit closes; testnet deploy can land after PR-D.

## Related

- Whitepaper §5: [`mutav/docs/mutav-whitepaper.en.md` §5](https://github.com/mutav-finance/mutav/blob/main/docs/mutav-whitepaper.en.md)
- Research:
  - [`mutav/research/01-Protocol/reserve-asset-and-onchain-ramp.md`](https://github.com/mutav-finance/mutav/blob/main/research/01-Protocol/reserve-asset-and-onchain-ramp.md)
  - [`mutav/research/03-Stellar-Soroban/pilot-architecture-on-stellar.md`](https://github.com/mutav-finance/mutav/blob/main/research/03-Stellar-Soroban/pilot-architecture-on-stellar.md)
  - [`mutav/research/06-Sources/etherfuse-stablebonds.md`](https://github.com/mutav-finance/mutav/blob/main/research/06-Sources/etherfuse-stablebonds.md)
  - [`mutav/research/06-Sources/sep-56.md`](https://github.com/mutav-finance/mutav/blob/main/research/06-Sources/sep-56.md)
  - [`mutav/research/06-Sources/openzeppelin-stellar-contracts-vault.md`](https://github.com/mutav-finance/mutav/blob/main/research/06-Sources/openzeppelin-stellar-contracts-vault.md)
  - [`mutav/research/99-Inbox/default-process-market-brief.md`](https://github.com/mutav-finance/mutav/blob/main/research/99-Inbox/default-process-market-brief.md) (informs Phase 2 DefaultManager design)
  - [`mutav/research/99-Inbox/default-process-regulatory-brief.md`](https://github.com/mutav-finance/mutav/blob/main/research/99-Inbox/default-process-regulatory-brief.md) (regulatory positioning; Art. 37 IV freedom from SUSEP cadence)
- Stellar primary sources verified:
  - [State archival](https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival)
  - [Stellar Asset Contract](https://developers.stellar.org/docs/tokens/stellar-asset-contract)
  - [Contract authorization](https://developers.stellar.org/docs/build/guides/auth/contract-authorization)
  - [Contract accounts guide](https://developers.stellar.org/docs/build/guides/contract-accounts)
  - [SEP-56 Tokenized Vault Standard](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0056.md)
- Adjacent contract: [`contracts/stage2/fund/src/lib.rs`](../../contracts/stage2/fund/src/lib.rs) — patterns this spec inherits (two-step admin, pause, replay guard, event shape)
- Operator-key custody: [`docs/specs/2026-05-31-operator-key-runbook-design.md`](./2026-05-31-operator-key-runbook-design.md)
- Stage-1 surface README: [`contracts/stage1/README.md`](../../contracts/stage1/README.md)
