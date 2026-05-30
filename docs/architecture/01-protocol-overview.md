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

Several terms are domain-overloaded across the two protocol repos. The repo split disambiguates them in code; this section pins them down for prose. **When this repo (mutav-stellar) says a term unqualified, it means the smart-contract / on-chain sense below.**

| Term | Smart-contract / on-chain sense (this repo) | Off-chain sense (on `mutav-app`) |
|---|---|---|
| **contract** | A Soroban Rust contract. Today there's one: the `Fund` in [`contracts/fund/`](../../contracts/fund/). | A **rental contract** — the lease agreement between agency and tenant. Database record + CRUD UI in `mutav-app/convex/contracts/`, `mutav-app/src/components/contracts/`, `mutav-app/src/app/(app)/contracts/`. Wholly unrelated to Soroban code. |
| **admin** | The Stellar **admin keypair** — signs `set_*`, `cover_default`, partner whitelist, `propose_admin`/`accept_admin`. Held by a hardware wallet inside `mutav-app/apps/admin/`. See [`02-actors-and-trust.md`](./02-actors-and-trust.md). | On `mutav-app` (Convex / Auth0 side): a **staff role** that reviews KYC/KYB submissions, manages internal users. Has no chain authority. |
| **operator** | The Stellar **operator keypair** — signs routine fund-runtime calls (`receive_payment`, `process_redemptions`, `add_yield`, `charge_mgmt_fee`, etc.). Held in KMS; invoked by Convex Actions on `mutav-app` via short-lived OIDC credentials. See [`02-actors-and-trust.md`](./02-actors-and-trust.md). | Same — there is no second sense of "operator" off-chain. |
| **treasury** | The Mutav **treasury account** — an off-chain Stellar account distinct from operator/admin; its keypair lives in `mutav-app`'s Convex backend and signs SEP-10/SEP-24 anchor flows only. See [`02-actors-and-trust.md`](./02-actors-and-trust.md). | Same. |
| **fund** | The MUTAV fund — a single Soroban contract instance with AUM, NAV, redemption queue. | Same — investor flows in `mutav-app/apps/fund/` (formerly `mutav-finance/mutav-fund`) interact with this. |

**Rule of thumb**: an unqualified "contract" or "admin" in **this repo's prose** refers to the smart-contract / Stellar-key sense. In **`mutav-app`'s code or docs**, the same word means the rental-agreement / staff-role sense. There is no in-repo ambiguity because neither sense appears in the other's codebase.

## Repo split

The protocol is delivered across **two repositories** (consolidated 2026-05-30 per [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57)) — separated by audit surface and change cadence:

- **`mutav-stellar`** (this repo) — the **protocol layer**: the `Fund` Rust contract (audit-gated, slow cadence) plus the TypeScript SDK published as `@mutav-finance/mutav-stellar`. Read-oriented SDK; composes chain reads and produces transaction XDRs for consumers to sign. **No daemons, no operator key custody, no UI.**
- **`mutav-app`** (sibling, "the web side") — Turborepo monorepo that holds everything else: the persona-app shells served on `*.mutav.finance` subdomains (agency / investor / admin / etc.) plus the **Mutav API** (Convex backend). The operator runtime that previously lived here as Bun daemons moves to **KMS-backed Convex Actions** on `mutav-app`; admin authority lives in a hardware wallet inside `mutav-app/apps/admin/`. Consumes this repo's SDK.

Dependency: `mutav-app` consumes `mutav-stellar`'s SDK; the direction does not reverse.

The standalone [`mutav-fund`](https://github.com/mutav-finance/mutav-fund) web3 portal (formerly the third repo in this split) is soft-deprecating into `mutav-app/apps/fund/` as part of the monorepo migration ([`mutav-fund#11`](https://github.com/mutav-finance/mutav-fund/issues/11), [`mutav-app#139`](https://github.com/mutav-finance/mutav-app/issues/139)). Until the fold-in completes, `mutav-fund/main` remains the live web3 portal.

**Boundary rule** (custody-locality, not a system-wide security guarantee): this repo's deployment is the on-chain contract + the published SDK. Operator and admin authority live on `mutav-app`; end-user custody (agencies, investors) is wallet-held. See [`02-actors-and-trust.md`](./02-actors-and-trust.md) for the full trust model, including off-chain routing surfaces a compromised consumer could affect without touching any key.

*Trade-offs of the split*: SDK release coordination + cross-repo CI gates between `mutav-stellar` and `mutav-app`. These are real costs; the benefit (audit-gating only the contract surface) is the trade we accept.

## Status (2026-05-30)

- Phase A — testnet, contract deployed, SDK scaffold on `main` (PR #21).
- **Daemon PRs #22–#27** — orphaned by [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57). Operator runtime moves to Convex Actions on `mutav-app`; proposed verdict for each PR in [`decisions/2026-05-30-daemon-prs-orphan-verdict.md`](./decisions/2026-05-30-daemon-prs-orphan-verdict.md).
- Mainnet — gated on the readiness checklist (issue #40), now reframed around the Convex-Action runtime.

## Sources

- Contract narrative: [`contracts/contract-introduction.md`](../../contracts/contract-introduction.md)
- Whitepaper (sibling repo): `mutav-finance/mutav` → `docs/whitepaper.md`
- Audit index: GitHub issue #49
