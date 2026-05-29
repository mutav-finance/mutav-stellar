# Architecture

Working documentation of how the MUTAV Stellar protocol fits together — contract, backend daemons, and off-chain rails.

## Read order

1. [`01-protocol-overview.md`](./01-protocol-overview.md) — what MUTAV is and why
2. [`02-actors-and-trust.md`](./02-actors-and-trust.md) — who can do what
3. [`03-contract.md`](./03-contract.md) — the on-chain `Fund` contract
4. [`04-off-chain-integrations.md`](./04-off-chain-integrations.md) — Etherfuse, PIX, partners
5. [`05-backend-daemons.md`](./05-backend-daemons.md) — the daemon stack
6. [`06-canonical-flows.md`](./06-canonical-flows.md) — sequence diagrams
7. [`07-deployment-topology.md`](./07-deployment-topology.md) — what runs where

First-time readers: read 01 → 06 in order for a complete mental model. Then drill into 03 / 05 as needed for implementation work.

## Conventions

- **`docs/specs/`** describes what's NEXT (designs, proposals).
- **`docs/plans/`** describes HOW (multi-step implementation playbooks).
- **`docs/architecture/`** describes what IS (the system as deployed / in flight).

Architecture files document the current state plus its known gaps. Each file ends with a "Known gaps" section that cross-links to filed GitHub issues. When an issue is resolved, the corresponding doc is updated in the same PR.

## Repo boundary — three repos

The protocol is delivered across three repos, separated by audit surface and change cadence:

| Concern | `mutav-stellar` (here) | [`mutav-app`](https://github.com/mutav-finance/mutav-app) | `mutav-invest` (forthcoming) |
|---|---|---|---|
| Smart contracts | yes | — | — |
| TS SDK (chain interface) | yes (published) | consumes | consumes |
| Operator daemons | yes (operator key only here) | — | — |
| Admin tooling | yes | — | — |
| Real-estate platform — agency dashboard, rental-contract mgmt, payment collection | — | yes (Auth0 + Convex) | — |
| Investor portal — fund data, deposit/redeem, NAV view, KYC | — | — | yes |
| Stack | Rust + Bun | Auth0 + Convex | Next.js 16 + Bun + Stellar wallet kit |
| Audience | protocol team | real-estate agencies | investors |
| Change cadence | contract: slow (audit gate); daemons: medium | medium | fast |

**Why three repos**: tight change control on the contracts. The Rust contract surface lives in `mutav-stellar` and moves slowly; the agency platform and investor portal iterate fast on their own schedules without dragging contract-grade rigor. *Trade-offs*: SDK release coordination across siblings, multi-repo CI gates, fragmented onboarding for newcomers, harder cross-cutting refactors.

**Boundary rule** (custody-locality, not a system-wide security guarantee): operator/admin custody never leaves `mutav-stellar`'s deployment. Agency and investor custody is end-user-owned and out of scope here. Both sibling repos consume the SDK to read chain state; users sign their own transactions client-side. See [`02-actors-and-trust.md`](./02-actors-and-trust.md) for the full trust model.

## Status snapshot (2026-05-29)

- **Phase A (testnet)**: contract deployed; backend daemons in PRs #22–#27 (audit follow-ups outstanding — 4 CHANGES_REQUESTED, 2 COMMENT per the per-daemon table in [`05-backend-daemons.md`](./05-backend-daemons.md)).
- **Phase B (mainnet)**: gated on the pre-mainnet readiness checklist (issue #40).
- **Audit**: see issue #49 for the global-audit index.

## Diagrams

All diagrams are mermaid (renders natively on GitHub, diffable as plain text).
