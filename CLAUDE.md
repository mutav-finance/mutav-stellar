@AGENTS.md

# MUTAV Stellar — Agent Context

## Project
MUTAV — onchain rental guarantee infrastructure.

This is the **Stellar contracts + operator infrastructure** for MUTAV. It houses the audited surface — strict change control because a bug here moves money. UI surfaces live in sibling repos.

Scope of this repo:
- Stellar/Soroban smart contracts (`Fund`)
- TypeScript SDK published as `@mutav-finance/mutav-stellar`
- Operator daemons (on-ramp, off-ramp, yield-sync, mgmt-fee, heartbeat, ttl-watchdog)
- Admin tooling for cold-wallet operations

Three-repo split:
- **`mutav-stellar`** (here): contracts + SDK + operator daemons + admin tooling. Tight change control. No UI.
- **`mutav-finance/mutav-app`**: real-estate platform — rental-contract management + agency payments. Stack: Auth0 + Convex. Audience: agencies. Consumes this repo's SDK.
- **`mutav-finance/mutav-invest`** (forthcoming): investor portal — fund data + dApp deposit/redeem flows. Audience: investors. Consumes this repo's SDK.

Both sibling repos consume this repo's SDK; neither feeds back into it. **Operator/admin keys never leave this repo's deployment.**

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
