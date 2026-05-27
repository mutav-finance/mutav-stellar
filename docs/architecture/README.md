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

## Status snapshot (2026-05-27)

- **Phase**: testnet — contract deployed; backend daemons in PRs #22–#27 (changes-requested per audit).
- **Phase B (mainnet)**: gated on the pre-mainnet readiness checklist (issue #40).
- **Audit**: see issue #49 for the global-audit index.

## Diagrams

All diagrams are mermaid (renders natively on GitHub, diffable as plain text).
