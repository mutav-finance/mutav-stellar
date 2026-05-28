@AGENTS.md

# MUTAV Stellar — Agent Context

## Project
MUTAV — onchain rental guarantee infrastructure.

This is the **protocol side** of MUTAV. It houses everything that requires operator or admin authority plus the public investor surface:
- Stellar/Soroban smart contracts
- TypeScript SDK consumed by all UIs
- Operator daemons (on-ramp, off-ramp, yield-sync, mgmt-fee, heartbeat, ttl-watchdog)
- Investor dApp (forthcoming) — public deposit/redeem UI, signs client-side

The **agency dashboard** lives in the sibling repo `mutav-finance/mutav-app` (UI only, no keys). It consumes this repo's SDK and reads the chain directly.

Boundary rule: operator/admin keys must never live in `mutav-app`.

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
