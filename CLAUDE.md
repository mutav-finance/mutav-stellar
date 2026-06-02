@AGENTS.md

# MUTAV Stellar — Agent Context

## Project
MUTAV — onchain rental guarantee infrastructure.

This is the **Stellar protocol layer** for MUTAV: the `Fund` Soroban contract plus the published TypeScript SDK. The audited surface. No daemons, no operator-key custody, no UI — those live on [`mutav-app`](https://github.com/mutav-finance/mutav-app).

## Research knowledge base (lives in `mutav`)

Protocol-wide continuous research lives in the sibling [`mutav`](https://github.com/mutav-finance/mutav) repo as an Obsidian vault at `mutav/research/` — five watch-streams (internal protocol changes, RWA/tokenization, Stellar/Soroban, Brazil market & regulation, security) with a daily auto-update. **Don't scatter research notes in this repo.** To add a source/reference (papers, audits, exploit write-ups, peer protocols), drop it in `../mutav/research/99-Inbox/intake.md` for triage — procedure in the `triage-source` skill at `../mutav/.claude/skills/`.

Scope of this repo:
- **Rust contract** (`contracts/`) — Soroban `Fund`. Audit-gated, slow cadence; the smallest changeable thing.
- **TS SDK** (`src/`) — published as `@mutav-finance/mutav-stellar`. Consumed by `mutav-app` for chain reads + transaction composition.

Two-repo split (per [`#57`](https://github.com/mutav-finance/mutav-finance/issues) — see [`docs/architecture/01-protocol-overview.md`](docs/architecture/01-protocol-overview.md#repo-split) for the canonical write-up):
- **`mutav-stellar`** (here): contract + SDK. Protocol-only.
- **`mutav-finance/mutav-app`**: Turborepo monorepo. Persona apps (agency / fund / admin / …) + the **Mutav API** (Convex backend). The operator runtime that used to live here (6 in-flight Bun daemons across PRs #22–#27) moves there as KMS-backed Convex Actions; orphan-verdict tracked at [`docs/architecture/decisions/2026-05-30-daemon-prs-orphan-verdict.md`](docs/architecture/decisions/2026-05-30-daemon-prs-orphan-verdict.md).

`mutav-app` consumes this repo's SDK; the dependency direction does not reverse.

**Boundary rule** (custody-locality, not a system-wide security guarantee): operator/admin keys do not live in this repo's deployment surface anymore — operator authority lives in a KMS-backed Convex Action on `mutav-app`, admin authority on a hardware wallet inside `mutav-app/apps/admin/`. This repo's deployment is the on-chain contract and the published SDK. See [`docs/architecture/02-actors-and-trust.md`](docs/architecture/02-actors-and-trust.md) for the full trust model — including off-chain routing surfaces (e.g. the agency app displaying payment addresses) that a compromised consumer could affect without touching any key.

## Terminology (overloaded across repos)

- **contract** here = Soroban smart contract (Rust). On `mutav-app` = rental contract (lease agreement, database record). Unrelated senses.
- **admin** here = Stellar admin keypair (cold wallet, signs `set_*` / `cover_default` / handover). On `mutav-app` = Auth0 staff role (KYC review, no chain authority).
- **operator**, **treasury**, **fund** are single-sense. Full table: `docs/architecture/01-protocol-overview.md#terminology`.

Part of the NearX acceleration program.

## Shared docs
Strategy, whitepaper, pitch deck, and brand assets live in a sibling repo.
Clone it for full context:

```bash
git clone https://github.com/mutav-finance/mutav.git ../mutav
```

Key files:
- `../mutav/docs/whitepaper.md` — protocol design and architecture
- `../mutav/docs/pitch-deck.md` — positioning and market context
- `../mutav/JURY.md` — evaluation criteria

If the sibling repo is not cloned locally, fetch files directly:
```bash
gh api repos/mutav-finance/mutav/contents/docs/whitepaper.md --jq '.content' | base64 -d
```

## Stack
- Stellar, Soroban SDK, Rust
- Branch workflow: feature branches → squash merge PRs to main

## Code standards
- `cargo fmt` and `cargo clippy` before pushing
