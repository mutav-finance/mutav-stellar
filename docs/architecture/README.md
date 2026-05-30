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

## Repo boundary — two repos

The protocol is delivered across two repos, separated by audit surface and change cadence (consolidated 2026-05-30 per [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57)):

| Concern | `mutav-stellar` (here) | [`mutav-app`](https://github.com/mutav-finance/mutav-app) |
|---|---|---|
| Smart contracts | yes | — |
| TS SDK (chain interface) | yes (published) | consumes |
| Operator runtime | — | yes (KMS-backed Convex Actions) |
| Operator key custody | — | yes (KMS) |
| Admin runtime | — | yes (hardware wallet inside `apps/admin/`) |
| Web surface — agency dashboard, investor portal, admin, marketing, docs | — | yes (Turborepo monorepo: persona apps on `*.mutav.finance`) |
| Mutav API — Convex backend orchestrating off-chain state | — | yes |
| Stack | Rust + Bun | Next.js 16 + Convex + Turborepo |
| Audience | protocol team | agencies, investors, protocol-team admin |
| Change cadence | contract: slow (audit gate); SDK: medium | medium-fast |

The standalone [`mutav-fund`](https://github.com/mutav-finance/mutav-fund) web3 portal soft-deprecates into `mutav-app/apps/fund/` during the upcoming monorepo migration ([`mutav-fund#11`](https://github.com/mutav-finance/mutav-fund/issues/11), [`mutav-app#139`](https://github.com/mutav-finance/mutav-app/issues/139)).

**Why two repos**: tight change control on the contracts. The Rust contract surface lives here and moves slowly; the web surface + operator runtime live in `mutav-app` and iterate without dragging contract-grade rigor through every change. *Trade-offs*: SDK release coordination across the two repos, cross-repo CI gates for SDK consumers.

**Boundary rule** (custody-locality, not a system-wide security guarantee): this repo's deployment is the on-chain contract + the published SDK. Operator and admin authority live on `mutav-app` (KMS-backed Convex Action and hardware wallet inside `apps/admin/` respectively); end-user custody (agencies, investors) is wallet-held and out of scope. See [`02-actors-and-trust.md`](./02-actors-and-trust.md) for the full trust model, including off-chain routing surfaces that a compromised consumer could still affect without touching any key.

## Status snapshot (2026-05-30)

- **Phase A (testnet)**: contract deployed; SDK on `main`. The 6 backend daemons in PRs #22–#27 are **orphaned** by the architecture consolidation in [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57) — see [`decisions/2026-05-30-daemon-prs-orphan-verdict.md`](./decisions/2026-05-30-daemon-prs-orphan-verdict.md) for the proposed disposition. Operator runtime moves to KMS-backed Convex Actions on `mutav-app`.
- **Phase B (mainnet)**: gated on the pre-mainnet readiness checklist (issue #40).
- **Audit**: see issue #49 for the global-audit index.

## Diagrams

All inline diagrams are mermaid (renders natively on GitHub, diffable as plain text). The target-state Excalidraw snapshot from the 2026-05-30 brainstorm is committed at [`diagrams/target-state.excalidraw.json`](./diagrams/target-state.excalidraw.json) — open via Excalidraw's File → Open dialog.
