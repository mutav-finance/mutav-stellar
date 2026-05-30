# 05 — Operational layer

Six operational responsibilities wrap the `Fund` contract for day-to-day operations. Each runs on its own cadence; together they cover partner-inflow recording, redemption cycling, yield/fee accrual, and TTL renewal.

Per [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57) the operational layer lives on **`mutav-app`** as Convex crons + Actions, not in this repo. Operator key custody moves from a daemon host to a KMS-backed signing pathway invoked by each Action with short-lived OIDC credentials.

> **Status (2026-05-30)**: the prior shape was 6 in-flight Bun daemons in this repo (`src/jobs/*.ts` per PRs [#22](https://github.com/mutav-finance/mutav-stellar/pull/22)–[#27](https://github.com/mutav-finance/mutav-stellar/pull/27)). Those PRs are orphaned by `#57` — see [`decisions/2026-05-30-daemon-prs-orphan-verdict.md`](./decisions/2026-05-30-daemon-prs-orphan-verdict.md) for the per-PR disposition. None of the runtime responsibilities below have shipped yet; the consolidation reframes them, it does not retire any.

## The six responsibilities

| # | Responsibility | Purpose | Cadence | Convex primitive |
|---|---|---|---|---|
| 1 | **on-ramp** | Poll Horizon for partner USDC payments → call `receive_payment` | 30 s poll | Convex cron + Action |
| 2 | **off-ramp** | Weekly redemption cycle: `process_redemptions` → Etherfuse liquidation → `fulfill_redemption` per investor | Weekly | Convex cron + Workflow (multi-step durability) |
| 3 | **yield-sync** | Record Etherfuse yield onto AUM via `add_yield`, batched against per-call cap | On-demand (admin-triggered or scheduled) | Convex Action |
| 4 | **mgmt-fee** | Monthly: `charge_mgmt_fee` on-chain + Classic-asset PIX payout | Monthly | Convex cron + Workflow (atomic split across on-chain + off-chain) |
| 5 | **heartbeat** | Renew contract instance TTL every 25 days | Every 25 d | Convex cron + Action |
| 6 | **ttl-watchdog** | Renew each investor's balance TTL via `extend_balance_ttl` | Every 25 d per investor | Convex cron + Action; iterate over the investor table |

Which Convex domain owns which Action is `mutav-app`'s call — see `mutav-app/docs/architecture/README.md` § Domain catalog and the planning ask in [`mutav-app#139`](https://github.com/mutav-finance/mutav-app/issues/139).

## State ownership

| Responsibility | On-chain state mutated | Off-chain state |
|---|---|---|
| on-ramp | `receive_payment` → AUM + protocol cut; `SeenTxHash` for replay protection | Horizon cursor (Convex table — replaces the prior `.on-ramp-cursor` file) |
| off-ramp | `process_redemptions` (queue → ready) + `fulfill_redemption` per investor | In-flight redemption set (Convex Workflow state — replaces the previously-missing daemon-side persistence) |
| yield-sync | `add_yield` (AUM↑) | None persistent; Action arguments parameterize the batch |
| mgmt-fee | `charge_mgmt_fee` (AUM↓) + Classic payment submission | Workflow checkpoint between on-chain debit and off-chain payout (resolves the atomic-split bug that the Bun-daemon design had) |
| heartbeat | `extend_ttl()` (instance) | Last-renewed timestamp (Convex table) |
| ttl-watchdog | `extend_balance_ttl(investor)` per investor | Per-investor renewal table (Convex; replaces the prior `data/ttl-watchdog.json`) |

## Coordination model

There is no Action-to-Action coordination beyond what the contract's per-function invariants enforce (replay-guard, 30-day mgmt-fee interval, weekly exit cap, TTL leases). This works for the responsibilities with disjoint state mutations (heartbeat, ttl-watchdog, yield-sync — each owns its slice of contract state).

It fails for the responsibilities that span an on-chain/off-chain transaction boundary (off-ramp, mgmt-fee). Those need durable in-flight state to recover from crashes between the on-chain side-effect and the off-chain side-effect. The Bun-daemon design left this as a gap (no persistence layer); the Convex-Action design closes it by using **Convex Workflow** for the multi-step Actions — Workflow handles the durability primitive natively.

## Why this lives on `mutav-app` and not here

Three reasons:

1. **Key custody**. The operator key has to live somewhere; KMS-backed Convex Actions are the target. This repo's deployment is the on-chain contract + the published SDK — no signing keys.
2. **Cadence**. The contract is audit-gated; the operational layer is not. Putting them in the same repo as the prior shape did meant every operational tweak inherited the contract repo's change-control overhead. Moving the operational layer to `mutav-app` lets it iterate at the Convex repo's normal cadence.
3. **Code reuse**. The operational layer needs to read off-chain data (Etherfuse yield, partner-payment context, KYC state) that `mutav-app`'s Convex tables already hold. Co-locating the Actions with that data eliminates a cross-repo data fetch.

## Known gaps

- **KMS-backed Convex Action runbook** — [`#41`](https://github.com/mutav-finance/mutav-stellar/issues/41). The KMS provider, OIDC trust policy, and per-Action scoped-key strategy are not yet documented.
- **Per-Action issues against `mutav-app`** — to be filed after the daemon-PR orphan verdict is accepted. Each issue references the original Bun-daemon PR (#22–#27) for the audit context that's worth porting.
- **Observability** — issue #44. Structured logs / metrics / pager wiring becomes Convex's native instrumentation plus the org's chosen alerting target.
- **Foundation modules** — the Bun-daemon design needed shared utilities (logger, retry, shutdown, env validation, bootstrap, nominal types) that issue #38 catalogued. Many of these don't apply under Convex (Convex provides logging + retry + shutdown semantics natively); the nominal types (`Usdc6`, `Stroops`, `ContractId`, `EpochMs`) remain useful and should live in the SDK (`src/`) so both this repo's tooling and the Convex Actions consume them — tracked at #37.
