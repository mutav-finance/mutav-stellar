# Mutav Guarantees Vault — Protocol Whitepaper

> **Status:** Draft. This document specifies the on-chain vault primitive implemented by the `Fund` Soroban contract in this repository. It is the audited surface of the MUTAV protocol; off-chain components (payment processing, partner whitelist, KYC, anchor flows, BRL rails, legal entities) live elsewhere and are referenced rather than defined here.
>
> **Audience:** Technical reviewers, security auditors, Soroban contributors. Reading-time target ≈ 60 minutes. Assumes Stellar / Soroban literacy; assumes ERC4626-style vault familiarity.
>
> **Companion documents:**
> - Business / GTM / legal entity structure: [`mutav/docs/whitepaper.md`](https://github.com/mutav-finance/mutav/blob/main/docs/whitepaper.md)
> - Actors and trust model: [`docs/architecture/02-actors-and-trust.md`](./architecture/02-actors-and-trust.md)
> - Canonical flows: [`docs/architecture/06-canonical-flows.md`](./architecture/06-canonical-flows.md)
> - Money flow diagram: [`docs/architecture/diagrams/money-flow.md`](./architecture/diagrams/money-flow.md)

---

## 1. Abstract

*[To be written]*

A one-paragraph formal definition of MGV: a single Soroban contract implementing a USDC-denominated vault with NAV-appreciating shares, an asynchronous redemption queue with periodic exit caps, and operator-mediated yield and default accounting. Pin the audited surface and the trust boundary in two more sentences.

---

## 2. Scope and non-goals

*[To be written]*

**In scope** — what the vault contract defines and enforces:
- Share-token accounting (SEP-41)
- NAV computation and conversion math
- Inflow accounting (`record_deposit`, `add_yield`, `add_tenant_fee`)
- Asynchronous redemption queue (`request_redemption` → `process_redemptions` → `fulfill_redemption` → `reclaim_expired_redemption`)
- Fee mechanics (`charge_mgmt_fee`, redemption fee on fulfill)
- Default coverage (`cover_default` — AUM debit)
- Authority model (admin / operator / treasury / classic_wallet / investor)
- Storage tier and TTL discipline
- Pause and admin handover

**Out of scope** — explicitly delegated to off-chain layers or sibling repos:
- Partner-imobiliária whitelist + KYC enforcement → [`mutav-app`](https://github.com/mutav-finance/mutav-app)
- BRL on-ramp, payment splitting, partner-payment ingestion → payment system on `mutav-app`
- Anchor flows (USDC ↔ TESOURO via Etherfuse) → off-chain Stellar Classic transactions
- Tranche composition / waterfall absorption between multiple vault instances → GTM layer
- Multi-chain implementations (`mutav-solana`) → parallel design, not specified here
- Legal entity structure (Mutav Soluções, Treasury Fund, Treasury Management) → [`mutav/docs/whitepaper.md`](https://github.com/mutav-finance/mutav/blob/main/docs/whitepaper.md)
- Operator-runtime key custody (KMS-backed Convex Actions) → [`docs/specs/2026-05-31-operator-key-runbook-design.md`](./specs/2026-05-31-operator-key-runbook-design.md)

---

## 3. Accounting model

*[To be written]*

State variables, invariants, storage placement.

- `aum: i128` (instance) — total assets under management, denominated in USDC stroops
- `supply: i128` (instance) — total MUTAV share supply
- `balance(Address): i128` (persistent) — per-investor share balance
- Invariant: `Σ balance(a) = supply` for all addresses `a`
- Invariant: `aum ≥ 0`, `supply ≥ 0`
- Invariant: `NAV = aum / supply` when `supply > 0`; `NAV₀ = 1.0` when `supply = 0`

Storage tier rationale per state shape.

---

## 4. Share token

*[To be written]*

The MUTAV share token as a SEP-41 implementation.

- `decimal = 7` (Stellar convention)
- `name`, `symbol` set at initialization
- Standard `transfer`, `approve`, `transfer_from`, `burn`, `burn_from`
- Why full SEP-41 (composability with DEXes and DeFi) vs. a restricted token (would force KYC enforcement on-chain)

---

## 5. NAV and conversion math

*[To be written]*

`calc_mint(amount_usdc, aum, supply)` and `calc_redeem(shares, aum, supply)` with rounding direction analysis.

- Mint rounds **down** in favor of the vault: investor receives floor of the proportional shares
- Redeem rounds **down** in favor of the vault: investor receives floor of the proportional USDC
- Share-inflation / donation attack vector: small first-depositor + direct USDC transfer can manipulate NAV; analyze whether `add_yield`'s `max_aum_increase_bps` mitigation closes this
- Reference: ERC4626 and the Cream Finance share-inflation incident

---

## 6. Inflow operations

*[To be written]*

Three inflow paths that credit AUM and may or may not mint shares.

| Operation | Authority | AUM | Supply | USDC moves on-chain |
|---|---|---|---|---|
| `record_deposit(amount)` | operator | `+amount` | `+calc_mint(amount, aum, supply)` to depositor | `depositor → contract → classic_wallet` |
| `add_yield(amount)` | operator | `+amount` (capped) | unchanged | none |
| `add_tenant_fee(amount)` | operator | `+amount` (capped) | unchanged | none |

Per-call cap (`max_aum_increase_bps`) protection model. What per-period rolling caps would add (open question for §13).

> **Note:** `record_deposit` replaces the current `deposit_investor` after the payment-system split (open decision §13a). Until that ships, the operative entrypoint is `deposit_investor` with identical semantics modulo the inflow-side accounting; see [`contracts/fund/src/lib.rs:503`](../contracts/fund/src/lib.rs).

---

## 7. Redemption mechanism

*[To be written]*

Formal model of the three-phase redemption cycle.

**Phase 1 — Request (investor-signed).**
`request_redemption(investor, mutav_amount)`:
- Investor balance decremented; shares not yet burned
- `PendingRedemption(investor) = mutav_amount` (persistent)
- `investor` appended to `RedemptionQueue` (FIFO)

**Phase 2 — Process (operator-signed, weekly cadence).**
`process_redemptions()`:
- Weekly epoch rollover when `now / WEEK_SECONDS > weekly_epoch`
- `cap = aum × exit_cap_bps / 10_000`
- `available = cap − weekly_exit_used`
- Walk queue up to `MAX_QUEUE_BATCH = 40` entries; for each:
  - If `usdc_out > available`: defer (carry forward, extend TTL on `PendingRedemption`)
  - Else: burn shares, debit AUM, write `ReadyRedemption(investor, deadline = now + fulfill_window_seconds)`

**Off-contract step — Pre-fund.**
`classic_wallet` redeems TESOURO at Etherfuse, transfers `Σ usdc_gross` of `ReadyRedemption` entries into the contract address via a plain SEP-41 `transfer` (signed by `classic_wallet`). Not a contract call; required before phase 3.

**Phase 3 — Fulfill (operator-signed) or reclaim (investor-signed).**
`fulfill_redemption(investor)`:
- Asserts `now ≤ deadline`
- `fee = gross × fee_bps / 10_000` (snapshot at process time)
- Transfers `gross − fee` to investor, `fee` to `protocol_addr`
- Removes `ReadyRedemption`

`reclaim_expired_redemption(investor)`:
- Asserts `now > deadline`
- Restores shares to investor, credits AUM back
- Removes `ReadyRedemption`

**Resource-budget derivation for `MAX_QUEUE_BATCH = 40`.** Reproduce the inline analysis at [`contracts/fund/src/lib.rs:14–31`](../contracts/fund/src/lib.rs) — Soroban read footprint (100 entries: 14 overhead + 86 / 2 per examined = 43, rounded to 40); write footprint (50 entries: 3p + (40−p) + 7 ≤ 50 → p ≤ 1 with full queue).

---

## 8. Fee mechanics

*[To be written]*

Two fee surfaces on-chain.

**Management fee** — `charge_mgmt_fee()`:
- Operator-authorized
- 30-day minimum interval (`MIN_FEE_INTERVAL`)
- `fee = aum × mgmt_fee_bps / 10_000`
- Pure AUM debit; no on-chain transfer (off-chain Classic USDC tx from `classic_wallet` to `treasury`, then off-chain BRL distribution)

**Redemption fee** — captured during `fulfill_redemption`:
- `fee = gross × ready_redemption.fee_bps / 10_000`
- `fee_bps` snapshotted at `process_redemptions` time — immune to later parameter changes
- Transferred USDC from contract to `protocol_addr` in the same tx

Why mgmt-fee distribution is off-chain (Soroban cannot produce Classic memos; treasury hand-off requires memo).

---

## 9. Default coverage

*[To be written]*

`cover_default(amount_usdc, destination)`:
- Admin-authorized
- Pure AUM debit
- `destination` logged in event for audit; does not receive on-chain USDC
- Off-chain BRL payout: `classic_wallet` redeems TESOURO → USDC → BRL via PIX → imobiliária bank

Trade-off: a fully on-chain payout would close the audit loop but requires (a) a way to encode bank-account routing (memo), (b) an USDC-to-BRL anchor that accepts contract-originated payments. Soroban supports neither cleanly. Open question §13g surfaces the cap + timelock discussion.

---

## 10. Authority model

*[To be written]*

Direct mapping: function → required signer.

| Authority | Functions |
|---|---|
| `admin` (`require_admin`) | `set_admin`, `propose_admin`, `accept_admin`, `set_operator`, `set_classic_wallet`, `set_approved_partner`, `set_*_bps`, `set_fulfill_window`, `set_paused`, `cover_default` |
| `operator` (`require_operator`) | `receive_payment`, `deposit_investor`, `process_redemptions`, `fulfill_redemption`, `add_yield`, `add_tenant_fee`, `charge_mgmt_fee`, `record_offchain_payout`, `sweep_usdc`, `extend_ttl` |
| `investor` (`investor.require_auth()`) | `request_redemption`, `cancel_redemption`, `reclaim_expired_redemption`, SEP-41 ops |
| `treasury` | none — passive recipient only |
| `classic_wallet` | none on-chain — signs off-contract USDC transfers (pre-fund + payouts) |

Two-step admin handover via `propose_admin` / `accept_admin` — atomic ownership transfer is rejected as the universal anti-pattern (typo or compromise → unrecoverable).

Soroban auth-chain semantics for USDC transfers. When operator calls `receive_payment`, the contract's call to `usdc_token.transfer(operator, contract, amount)` derives auth from the operator's outer `require_auth()` — no separate operator allowance to the contract is needed.

Off-chain custody story per role lives in [`docs/architecture/02-actors-and-trust.md`](./architecture/02-actors-and-trust.md); not re-derived here.

---

## 11. Storage, TTL, and resource bounds

*[To be written]*

Three tiers and their use.

**Instance storage** — single-cell, always loaded:
- Governance: `Admin`, `Operator`, `ProtocolAddr`, `ClassicWallet`
- Accounting: `Aum`, `TotalSupply`, `LastFeeTimestamp`
- Config (set once at init, read-only after): `ExitCapBps`, `MgmtFeeBps`, `RedemptionFeeBps`, `ProtocolFeeBps`, `MaxAumIncreaseBps`, `FulfillWindowSeconds`
- Weekly cap state: `WeeklyEpoch`, `WeeklyExitUsed`
- Token metadata: `TokenMeta`
- Pause + handover: `Paused`, `PendingAdmin`

**Persistent storage** — per-key, archivable + restorable:
- Per-investor: `Balance(Address)`, `Allowance(AllowanceKey)`
- Redemption queue: `PendingRedemption(Address)`, `ReadyRedemption(Address)`, `RedemptionQueue`
- Partner whitelist: `ApprovedPartner(Address)`

**Temporary storage** — auto-deleted at TTL expiry, not restorable:
- Replay guard: `SeenTxHash(BytesN<32>)` — 7-day TTL (~50,400 ledgers at 12s spacing)

TTL discipline:
- Persistent entries extend to ~30 days (518,400 ledgers) on every touch
- Instance extends on every state-mutating call
- Temporary `SeenTxHash` set once, never extended — replay guard expires safely after 7 days

Resource bounds for `process_redemptions` reproduced from §7.

Failure mode: TTL exhaustion. If an investor's `Balance(Address)` archives, restoring it requires their action (or an admin-paid restoration). The off-chain operator runtime is expected to extend persistent TTLs proactively via per-investor watchdog calls (`extend_balance_ttl` — implemented as part of #69 operator CLI scope).

---

## 12. Adversarial properties

*[To be written]*

Invariants and what holds under each attacker class. This section is **technical**: it lists which invariants the contract enforces against on-chain adversaries, not how the operations team defends against off-chain compromise.

**Attacker classes considered:**
1. External caller (no authority)
2. Approved partner with valid `tx_hash` collisions
3. Compromised operator key
4. Compromised admin key
5. Malicious USDC SAC (trust assumption — out of scope)
6. Malicious investor (own funds)

**Per-class:** which contract invariants degrade and how. The known security gaps (`sweep_usdc` reserve check #28; `cover_default` cap #30; `add_yield` per-period cap #31; `set_classic_wallet` single-step #32; `record_offchain_payout` destination whitelist #33) get one paragraph each, mapped to which adversary they affect.

Out of section scope: physical key custody, social-engineering, regulatory action, infrastructure compromise. Those live in `02-actors-and-trust.md`.

---

## 13. Open questions

*[To be written]*

Each subsection presents the question, the options, the trade-offs, and a **Decision:** line. The point of this section is to make the writing process drive the architecture choices.

- **13a — Payment-system split.** Move `receive_payment`, `protocol_fee_bps`, `protocol_addr`, `ApprovedPartner`, `SeenTxHash`, `set_approved_partner` out of the vault into a separate payment-system layer? Trade-off: smaller audit surface vs. more inter-system coordination.
- **13b — `classic_wallet` keypair custody.** Currently undocumented in `02-actors-and-trust.md`. Options: (i) KMS-backed Convex Action sibling of operator, (ii) separate scoped key, (iii) HW wallet with operator runtime calling out for signatures.
- **13c — `__constructor` migration.** Move `initialize` → Soroban Protocol 22 `__constructor` for atomic deploy + initialization, eliminating the deploy-then-frontrun window.
- **13d — `usdc_token` rotation path.** Add admin-authorized `set_usdc_token` for SAC migrations, or commit to immutability with redeploy on rotation.
- **13e — Per-period rolling AUM-credit cap.** `add_yield` / `add_tenant_fee` only have per-call caps. Adversarial-operator scenario: repeated max-cap calls. Add a rolling window cap.
- **13f — `sweep_usdc` reserve check.** Today sweep can drain USDC reserved for ready redemptions. Add explicit reserve subtraction.
- **13g — `cover_default` cap + timelock.** Today admin can call `cover_default` with no cap. Add a per-period cap (similar to `exit_cap_bps`) and optionally a timelock.
- **13h — Replay-guard TTL semantics.** `SeenTxHash` lives in temporary storage with 7-day TTL. After expiration, an operator with the same `tx_hash` could replay. Long-tail risk; specify whether 7 days is sufficient.

---

## 14. References

External sources are kept as one file per reference under [`docs/references/`](./references/). Each file contains: canonical URL, 1-line summary, relevance to MGV, annotated takeaways, key quotations, and cross-references to the MGV section that cites the source. The pattern keeps citations portable (other docs can link to the same notes) and review-friendly (auditors can read a single source without scrolling through prose).

**Standards and specifications:**
- [`references/erc-4626.md`](./references/erc-4626.md) — ERC4626 Tokenized Vault Standard
- [`references/sep-41.md`](./references/sep-41.md) — Stellar Token Interface
- [`references/soroban-storage-tiers.md`](./references/soroban-storage-tiers.md) — Soroban storage tier semantics
- [`references/soroban-auth.md`](./references/soroban-auth.md) — Soroban authorization model

**Audited / production vault designs for comparison:**
- [`references/enzyme-finance.md`](./references/enzyme-finance.md) — Enzyme Finance protocol (asset-management fund accounting, fee policies, share-token semantics)
- [`references/yearn-v2.md`](./references/yearn-v2.md) — Yearn V2 vault architecture
- [`references/morpho-blue.md`](./references/morpho-blue.md) — Morpho Blue minimal-vault primitive
- [`references/lido-withdrawal-queue.md`](./references/lido-withdrawal-queue.md) — Lido async redemption queue
- [`references/compound-ctoken.md`](./references/compound-ctoken.md) — Compound cToken exchange-rate model
- [`references/openzeppelin-stellar-contracts-vault.md`](./references/openzeppelin-stellar-contracts-vault.md) — OpenZeppelin Stellar Contracts vault primitive (when available)

**Known vault vulnerability literature:**
- [`references/cream-finance-inflation-incident.md`](./references/cream-finance-inflation-incident.md) — Cream Finance share-inflation incident (Oct 2021)
- [`references/openzeppelin-virtual-shares.md`](./references/openzeppelin-virtual-shares.md) — OpenZeppelin virtual-shares mitigation analysis

**Mutav internal references:**
- [`docs/architecture/01-protocol-overview.md`](./architecture/01-protocol-overview.md)
- [`docs/architecture/02-actors-and-trust.md`](./architecture/02-actors-and-trust.md)
- [`docs/architecture/03-contract.md`](./architecture/03-contract.md)
- [`docs/architecture/06-canonical-flows.md`](./architecture/06-canonical-flows.md)
- [`docs/architecture/diagrams/money-flow.md`](./architecture/diagrams/money-flow.md)
- [`docs/specs/2026-05-31-operator-key-runbook-design.md`](./specs/2026-05-31-operator-key-runbook-design.md)
- [`contracts/fund/src/lib.rs`](../contracts/fund/src/lib.rs) — the audited source

---

## Document history

| Date | Version | Change |
|---|---|---|
| *TBD* | 0.1 | Initial scaffold |
