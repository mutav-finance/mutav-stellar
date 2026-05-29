@AGENTS.md

# MUTAV Stellar — Agent Context

## Project
MUTAV — onchain rental guarantee infrastructure.

This is the **Stellar contracts + operator infrastructure** for MUTAV. Two distinct surfaces under strict change control because a bug here moves money. UI surfaces live in sibling repos.

Scope of this repo:
- **Rust contract** (`contracts/`) — Soroban `Fund`. Audit-gated, slow cadence; the smallest changeable thing.
- **TS SDK + operator daemons** (`src/`) — published as `@mutav-finance/mutav-stellar`; daemons (on-ramp, off-ramp, yield-sync, mgmt-fee, heartbeat, ttl-watchdog) hold operator keys. Not "audited" in the same sense the contract is; needs its own change-control regime.
- **Admin tooling** for cold-wallet operations.

Three-repo split:
- **`mutav-stellar`** (here): contracts + SDK + operator daemons + admin tooling.
- **`mutav-finance/mutav-app`**: real-estate platform — rental-contract management + agency payments. Stack: Auth0 + Convex. Audience: agencies.
- **`mutav-finance/mutav-fund`**: investor portal — fund data + dApp deposit/redeem. Stack: Next.js 16 + Bun + Stellar wallet kit. Audience: investors.

Both sibling repos consume this repo's SDK; neither feeds back into it.

**Boundary rule** (custody-locality, not a security guarantee): operator/admin custody never leaves `mutav-stellar`'s deployment. Agency and investor custody is end-user-owned and out of scope for this repo. See `docs/architecture/02-actors-and-trust.md` for the full trust model — including off-chain routing surfaces (e.g. mutav-app displaying agency-payment addresses) that a compromised sibling could affect without touching operator keys.

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
