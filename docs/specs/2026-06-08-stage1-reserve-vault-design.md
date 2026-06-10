# Stage-1 reserve vault — design

**Date:** 2026-06-08 (last revised: 2026-06-09 — simplified to safe-with-allowlists; all policy lives in the OZ Smart Account at the admin address)
**Branch:** `docs/stage1-reserve-vault-design`
**Status:** Draft for review.
**Target crate:** [`contracts/stage1/reserve_vault/`](../../contracts/stage1/reserve_vault/)

## Headline

A custodial safe with two allowlists. Mutav's admin address is an OpenZeppelin Smart Account whose Context Rules carry the policy (signer thresholds, spending limits, timelocks, per-operation differentiation). The vault contract itself does **none** of that. It holds value, accepts admin-authorized withdrawals to whitelisted destinations of whitelisted assets, and emits events.

**This split is the safety story.** The vault is small enough to audit in a sitting. The Smart Account configuration is audited separately. Each evolves on its own cadence without touching the other.

## Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│ OZ Smart Account (lives at the vault's `admin` address)                │
│                                                                         │
│ Context Rules decide, for every call:                                  │
│   • Signer threshold (M-of-N) — per function, per amount               │
│   • Spending limits — cumulative per period, per asset                 │
│   • Timelocks — optional delay for high-value or sensitive ops         │
│   • Destination policies — agency vs. operator-wallet differentiation  │
│                                                                         │
│ Configured via Smart Account Kit SDK from `mutav-app/apps/admin/`      │
└────────────────────────────┬───────────────────────────────────────────┘
                              │ require_auth resolves here
                              ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Reserve Vault (this contract — minimal safe)                           │
│                                                                         │
│ • Holds SEP-41 token balances                                          │
│ • Admin-managed asset allowlist (≤ 8)                                  │
│ • Admin-managed destination allowlist (≤ 64)                           │
│ • Single value-flow path:                                              │
│     withdraw(asset, amount, destination, ref_hash)                     │
│       — admin auth                                                      │
│       — asset must be in allowlist                                      │
│       — destination must be in allowlist                                │
│       — amount > 0                                                      │
│       — not paused                                                      │
│                                                                         │
│ • Governance: add/remove approved asset, add/remove destination,       │
│   set_paused, two-step admin handover                                  │
│                                                                         │
│ • Off-chain: deposits arrive via plain SEP-41 transfers; portal scans  │
│   Stellar ledger for inbound, no contract function needed              │
└────────────────────────────────────────────────────────────────────────┘
```

## What's in the contract — complete surface

**Storage (instance, 5 keys):**

```rust
enum DataKey {
    Admin,                      // Address (OZ Smart Account in production)
    PendingAdmin,               // Address (two-step handover)
    Paused,                     // bool
    ApprovedAssets,             // Vec<Address> (≤ 8)
    AllowedDestinations,        // Vec<Address> (≤ 64)
}
```

No persistent storage. No temporary storage. No per-asset structs. No PendingSwap. No PayDefault lifecycle. No rate table. No denomination concept.

**Entry points (12 total):**

```rust
// Init
fn initialize(admin: Address);

// Admin governance
fn set_paused(paused: bool);
fn propose_admin(new_admin: Address);
fn accept_admin();                              // called by pending admin

// Asset allowlist
fn add_approved_asset(asset: Address);
fn remove_approved_asset(asset: Address);       // requires balance == 0

// Destination allowlist
fn add_allowed_destination(addr: Address);
fn remove_allowed_destination(addr: Address);

// The one value-flow path
fn withdraw(asset: Address, amount: i128, destination: Address, ref_hash: BytesN<32>);

// Views (read-only)
fn admin() -> Address;
fn pending_admin() -> Option<Address>;
fn paused() -> bool;
fn approved_assets() -> Vec<Address>;
fn is_approved_asset(asset: Address) -> bool;
fn allowed_destinations() -> Vec<Address>;
fn is_destination_allowed(addr: Address) -> bool;
fn balance(asset: Address) -> i128;             // reads SEP-41 token::balance(self)
```

That's the whole contract. ~250 LOC, **6.9 KB WASM** (vs. 19.5 KB before simplification).

## Events

| Event | Topic | Data |
|---|---|---|
| `withdraw` | `(symbol "withdraw", ref_hash)` | `(asset, amount, destination)` |
| `set_paus` | `(symbol "set_paus",)` | `paused: bool` |
| `prop_adm` | `(symbol "prop_adm",)` | `new_admin: Address` |
| `acc_adm` | `(symbol "acc_adm",)` | `new_admin: Address` |
| `asset_add` / `asset_rm` | `(symbol "asset_add",) / (symbol "asset_rm",)` | `asset: Address` |
| `dest_add` / `dest_rm` | `(symbol "dest_add",) / (symbol "dest_rm",)` | `addr: Address` |

`ref_hash` on `withdraw` is an opaque 32-byte off-chain reference (e.g., `(guarantee_contract_hash, covered_month)` hash, Etherfuse `mintBond` op id hash, Pix `EndToEndId` hash). The contract doesn't interpret it; the off-chain indexer correlates against Convex state to reconstruct per-guarantee, per-month payment history.

## What lives in the OZ Smart Account configuration (not in this contract)

These are **Context Rules** set up via Smart Account Kit SDK in `mutav-app/apps/admin/`:

| Concern | Smart Account Context Rule |
|---|---|
| Pay-default to an agency | Rule on `withdraw` call where destination is an agency address: 3-of-5 admin multisig + optional 24h timelock |
| Operator-managed Etherfuse subscribe | Rule on `withdraw` to operator-wallet destination: operator-class signer alone (no admin threshold) |
| Per-asset spending limit per period | Spending-limit policy on `withdraw` per asset per 30 days |
| Per-call value ceiling | Spending-limit policy on `withdraw` per call |
| Emergency pause | Rule on `set_paused(true)`: 1-of-5 (any admin can pause) |
| Unpause | Rule on `set_paused(false)`: 3-of-5 (consensus required to resume) |
| Allowlist mutations | Rule on `add_*` / `remove_*`: 4-of-5 (changing trust set) |
| Admin rotation | Rule on `propose_admin` / `accept_admin`: 5-of-5 (unanimous) |

The vault is **agnostic** to all of this. It calls `require_auth(admin)`. The Smart Account's `__check_auth` evaluates the call against the configured rules and authorizes or rejects.

## What's deliberately NOT in the vault contract

| Was considered, deliberately deferred to OZ Smart Account or off-chain |
|---|
| Pay-default propose/execute lifecycle with timelock |
| Operator role (second auth path) |
| Per-asset payment caps |
| Per-period cumulative caps |
| `PendingSwap` tracking for in-flight Etherfuse swaps |
| `record_capital_receipt` / `record_swap_in` (deposit logging) |
| `publish_snapshot` |
| Multi-month coverage tracking with `covered_month` |
| `guarantee_contract_hash` typing (folded into opaque `ref_hash`) |
| Rate table / denomination asset / valuation math |
| Multi-tier role split (admin / platform operator / fund operator) |

Each was reasonable on its own. Cumulatively they were a stack of policy logic that belongs in the Smart Account, not the vault. The vault stays a safe.

## Deposits

Value arrives at the vault via plain SEP-41 transfers from external wallets — PSP-routed reserve allocation, pre-seed wallet top-ups, operator-driven Etherfuse redemption results. The vault has **no `deposit` function**.

The Convex transparency portal subscribes to SEP-41 `transfer` events targeting the vault address, correlates them against expected inflows in Convex (which knows which guarantee a tenant payment is for), and presents the audit trail. The chain-level truth is the SAC `transfer` event; the contract-level event would be redundant.

## Withdrawals

Every value-out from the vault is one call:

```rust
withdraw(asset, amount, destination, ref_hash)
```

Whatever the **reason** for the withdrawal — paying an agency for a guarantee default, routing USDC to an operator wallet for an Etherfuse subscription, executing a Phase-2 fund redemption — it's the same on-chain primitive. The Smart Account's Context Rules differentiate the cases:

- Withdraw to agency address `G_ABC...` (in the destination allowlist) for amount X → applies the agency-payment policy (multisig + amount limit + maybe timelock)
- Withdraw to operator wallet `G_OPS...` (in the destination allowlist) for amount Y → applies the operational-routing policy (single operator signer + tighter amount limit + no timelock)

The vault doesn't know or care about the distinction. The Smart Account does.

## Trust model

| Authority | Identity | What they can do |
|---|---|---|
| **Admin** (the OZ Smart Account at the configured address) | OZ Smart Account contract, M-of-N passkey threshold, with Context Rules per operation | Everything that requires `require_auth(admin)`: every `withdraw`, every allowlist mutation, `set_paused`, `propose_admin`. Smart Account's rules govern who/when/how. |
| **Public verifier** | Anyone with Stellar RPC | Read all views (admin, balances, allowlists, paused state). Subscribe to vault events for the audit trail. |
| **Pre-seed / PSP / operator wallets** | External Mutav-controlled accounts | Send SEP-41 transfers to the vault address. No contract auth needed for inbound. |

Compromise model:

- **Vault contract bug** — small enough to audit completely. Per-tx limits don't exist at the vault layer, but the destination allowlist means even a faulty `withdraw` can only route to admin-approved destinations.
- **Smart Account contract bug** — the load-bearing dependency. Mitigated by focused secondary audit on the pinned OZ Smart Account version. The Smart Account is replaceable via `propose_admin` → `accept_admin` if a critical issue surfaces.
- **Smart Account misconfiguration** — admin team's responsibility. Periodic configuration review part of operations runbook.
- **Multiple key compromise (3-of-5 simultaneously)** — outside the threat model; assumed adequately defended by passkey-per-device separation.

## Verified Stellar primitives

Cross-checked against developers.stellar.org during the 2026-06-09 design session (full notes in earlier revisions of this spec):

- **SAC (CAP-46-6)** for asset interaction; `TokenClient::balance` and `TokenClient::transfer` are the only primitives the vault uses
- **Storage tiers** — only `instance` storage; no persistent or temporary needed at this size
- **Contract authorization** — `require_auth(admin)` resolves correctly whether admin is a keypair or a contract account (the OZ Smart Account is the latter)
- **`CustomAccountInterface` `__check_auth` signature** — what the OZ Smart Account implements to do its policy logic
- **OZ Stellar Contracts v0.7.2** — labeled "experimental software" by OZ; admin-side OZ Smart Account is the load-bearing dependency, included in Mutav's audit scope

## Goals

1. **Auditable in hours, not weeks.** The contract is ~250 LOC with no complex state machines, no rate math, no lifecycle. An auditor reads the whole thing in one sitting.
2. **Policy is configurable without contract changes.** The Smart Account's Context Rules can evolve (new spending limits, new destination policies, multi-tier signer classes) without ever touching this contract.
3. **Honest separation: vault holds, Smart Account decides.** The contract claims no policy. Compromise modeling decomposes cleanly: vault bug vs. Smart Account bug vs. misconfiguration.
4. **§5.4.3 transparency story preserved.** The portal sees `withdraw` events + ledger-level SEP-41 transfers + Smart Account audit log. The vault contract publishes raw balance views. K ratio computation is off-chain (always was).

## Non-goals

1. **No pay-default semantics in the vault.** All claim-payment policy lives in the Smart Account.
2. **No operator role at the contract layer.** Operator-class authority is a Smart Account configuration (lower-threshold signer class with restricted Context Rules).
3. **No on-chain rate, denomination, or valuation math.**
4. **No PendingSwap tracking.** Operators manage Etherfuse subscribe/redeem cycles via Smart Account-authorized `withdraw` calls. Off-chain pairs the inbound TESOURO arrival to the outbound USDC.
5. **No deposit function.** Inbound is SEP-41 transfers; portal scans the ledger.
6. **No snapshot publication function.** Portal queries balance views directly.

## Tests

16 tests covering the entire surface:

- `initialize_sets_admin_and_defaults`
- `initialize_cannot_be_called_twice`
- `add_remove_approved_asset`
- `add_duplicate_asset_panics`
- `remove_asset_with_balance_panics`
- `add_remove_destination`
- `add_duplicate_destination_panics`
- `withdraw_to_whitelisted_destination_succeeds`
- `withdraw_to_non_whitelisted_destination_panics`
- `withdraw_of_unapproved_asset_panics`
- `withdraw_zero_amount_panics`
- `withdraw_blocked_when_paused`
- `unpause_resumes_withdrawals`
- `two_step_admin_handover`
- `accept_admin_without_pending_panics`
- `balance_view_reads_sep41`

All pass. fmt + clippy -D warnings + wasm32v1-none build all clean.

## Open items (operational, not contract)

- **OZ Smart Account configuration** — lives in `mutav-app/apps/admin/`. Defining the Context Rules (which thresholds for which operations, spending limits, timelock policies, agency vs. operator destination differentiation). Out of this repo's scope; cross-references in the spec.
- **Etherfuse mainnet shadow** — operator-direct subscribe/redeem cycle with small capital to characterize the real flow before vault-integrated mainnet.
- **Focused audit on the pinned OZ Smart Account version** — bounded engagement, part of pre-mainnet checklist.
- **Trustline preflight in admin dashboard** — required for any agency receiving a `withdraw`. Lives in `mutav-app`.
- **Per-member out-of-band tx verification** — procedural control against dashboard-compromise blind-signing. Operations runbook.

## What changed from the prior revisions (history of this branch)

The branch went through five major revisions before landing here:

1. Initial spec with on-chain denomination + per-asset rate table
2. Refactor to per-asset caps (dropped denomination + rates)
3. Added on-chain destination allowlist for operator_outbound
4. Considered adding agency allowlist, per-period caps, operator daily caps for 50k scale
5. **Pivoted to this simplification:** all policy in the OZ Smart Account; vault is a safe with two allowlists

The earlier complexity was reasonable as an honest attempt to enforce policy on-chain. The simplification recognizes that OZ Smart Account's Context Rules are the right place for policy — they're configurable, replaceable, and explicitly designed for this. Putting the same logic in the vault contract was duplication that grew the audit surface without adding security beyond what the Smart Account provides.

## Phasing

| PR | Work | Branch |
|---|---|---|
| **PR-A** (this PR) | Spec + minimal vault contract + 16 tests + simulation doc update | `docs/stage1-reserve-vault-design` (PR #97) |
| **PR-B** | Verified Stellar primitives research updates (in `mutav` repo) | new branch in sibling repo |
| **PR-C** | Stage-2 `cover_default` → `pay_default` rename | separate mutav-stellar PR |
| **PR-D** | SDK additions in `src/providers/soroban/reserve_vault.ts` — XDR builders for the 12 entry points | follow-up after PR-A merges |
| **PR-E** (mutav-app side) | Admin dashboard with Smart Account Kit + Context Rules configuration | out of this repo |
