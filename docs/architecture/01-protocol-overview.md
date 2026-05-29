# 01 — Protocol overview

MUTAV is an onchain rental-guarantee fund operating on Stellar (Soroban). It connects three flows of money:

1. **Brazilian real-estate agencies** (`imobiliárias`) pay monthly guarantee fees in USDC to the operator wallet. The operator triggers the contract's `receive_payment`, which splits each payment (protocol share + AUM portion) and routes the AUM portion to the fund's `classic_wallet` — where Etherfuse converts it to a yield-bearing reserve.
2. **Investors** deposit USDC to receive MUTAV shares (a SEP-0041 token). Share value (NAV) grows as the reserve earns yield.
3. **Tenant defaults** (`sinistros`) are covered by debiting the reserve and paying the affected agency off-chain.

The fund's reserve is held off-chain in **Brazilian Treasury bonds (TESOURO)** via Etherfuse, an external provider that converts USDC ↔ TESOURO and reports yield. The Soroban contract tracks AUM, mints/burns shares, and gates exits via a weekly-capped redemption queue.

## What's on-chain vs off-chain

| Layer | What lives there |
|---|---|
| **Stellar Soroban** | The `Fund` contract: MUTAV token, AUM accounting, redemption queue, fee math, admin/operator roles, partner whitelist |
| **Stellar Classic** | USDC payments to/from partner agencies; PIX-MEMO classic-asset payouts for management fees |
| **Off-chain (Etherfuse)** | USDC ↔ TESOURO conversion; yield accrual on the reserve |
| **Off-chain (banking rails)** | PIX for fiat in/out; partner-agency bank accounts |

## Two control planes

- **Admin (cold wallet)**: parameter changes, partner whitelist, `cover_default`, admin handover. Used infrequently; intended to be a multisig / hardware wallet.
- **Operator (hot wallet)**: day-to-day ops — recording partner payments, recording yield, charging mgmt fee, processing redemptions, fulfilling redemptions, sweeping idle USDC. Used by the daemon stack.

The hot/cold split is enforced on-chain via `require_auth` on each function. Key custody policy is **not yet enforced at the host layer** — see issue #41.

## Tokens involved

- **USDC** (Stellar SAC): inflows from partners and investors; outflows for redemptions and sweeps.
- **MUTAV** (SEP-0041, this contract): fund shares. NAV = AUM / supply (USDC per MUTAV).
- **TESOURO** (Classic asset, issued by Etherfuse): off-chain reserve representation. Held in `classic_wallet` after conversion.

## Terminology

Several terms are domain-overloaded across the three repos. The repo split disambiguates them in code; this section pins them down for prose. **When this repo (mutav-stellar) says a term unqualified, it means the smart-contract / on-chain sense below.**

| Term | Smart-contract / on-chain sense (this repo) | Off-chain sense (sibling repos) |
|---|---|---|
| **contract** | A Soroban Rust contract. Today there's one: the `Fund` in [`contracts/fund/`](../../contracts/fund/). | On `mutav-app`: a **rental contract** — the lease agreement between agency and tenant. Database record + CRUD UI (`convex/contracts/`, `src/components/contracts/`, `src/app/(app)/contracts/`). Wholly unrelated to Soroban code. |
| **admin** | The Stellar **admin keypair** (cold wallet) — signs `set_*`, `cover_default`, partner whitelist, `propose_admin`/`accept_admin`. See [`02-actors-and-trust.md`](./02-actors-and-trust.md). | On `mutav-app`: an Auth0 **staff role** — reviews KYC/KYB submissions, manages internal users. Has no chain authority. |
| **operator** | The Stellar **operator keypair** (hot wallet) — signs daemon-triggered on-chain calls. See [`02-actors-and-trust.md`](./02-actors-and-trust.md). | — (does not appear in the sibling repos) |
| **treasury** | The Mutav **treasury account** — an off-chain Stellar account distinct from operator/admin; its keypair lives in `mutav-app`'s Convex backend and signs SEP-10/SEP-24 anchor flows only. See [`02-actors-and-trust.md`](./02-actors-and-trust.md). | Same. |
| **fund** | The MUTAV fund — a single Soroban contract instance with AUM, NAV, redemption queue. | Same — investor flows on `mutav-fund` interact with this. |

**Rule of thumb**: an unqualified "contract" or "admin" in **this repo's prose** refers to the smart-contract / Stellar-key sense. In **`mutav-app`'s code or docs**, the same word means the rental-agreement / Auth0-staff sense. There is no in-repo ambiguity because neither sense appears in the other's codebase.

## Repo split

The protocol is delivered across three repositories — separated by audit surface and change cadence:

- **`mutav-stellar`** (this repo) — Stellar contracts + operator infrastructure. Houses two surfaces with different discipline: the **Rust contract** (audit-gated, slow) and the **TS SDK + operator daemons** (operator-authority code; not "audited" in the same sense). Plus admin tooling. No UI.
- **`mutav-app`** (sibling, "real-estate platform") — agency-facing SaaS for rental-contract management and agency payment flows. Stack: Auth0 + Convex. Consumes this repo's SDK to read chain state. Surfaces "pay USDC to wallet X" instructions to agencies; agencies sign with their own wallets.
- **`mutav-fund`** (sibling, "web3 portal") — public dApp serving two audiences via wallet-signed transactions. **Investors**: deposit, request redemption, claim, NAV/portfolio view, KYC. **Protocol team (admin)**: dashboard, partner whitelist, parameter changes, `cover_default`, pause toggle, admin handover. Stack: Next.js 16 + Bun + Stellar wallet kit. Consumes this repo's SDK. No server-side keys; admin features gated by on-chain `admin()` check.

Dependencies: both sibling repos consume `mutav-stellar`'s SDK; neither feeds back.

**Boundary rule** (custody-locality, not a system-wide security guarantee): operator/admin custody never leaves this repo's deployment. Agency and investor custody is end-user-owned and out of scope here. See [`02-actors-and-trust.md`](./02-actors-and-trust.md) for the full trust model, including off-chain routing surfaces a compromised sibling could affect.

*Trade-offs of three repos*: SDK release coordination across siblings, multi-repo CI gates, fragmented onboarding for newcomers, harder cross-cutting refactors. These are real costs; the benefit (tight change control on the contracts) is the trade we accept.

## Status (2026-05-29)

- Phase A — testnet, contract deployed, backend scaffold landed (PR #21).
- Phase B — 6 backend daemons in flight (PRs #22–#27), audit follow-ups outstanding (4 CHANGES_REQUESTED, 2 COMMENT — see per-daemon table in [`05-backend-daemons.md`](./05-backend-daemons.md)).
- Mainnet — gated on the readiness checklist (issue #40).

## Sources

- Contract narrative: [`contracts/contract-introduction.md`](../../contracts/contract-introduction.md)
- Whitepaper (sibling repo): `mutav-finance/mutav` → `docs/whitepaper.md`
- Audit index: GitHub issue #49
