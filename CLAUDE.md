# SGR Stellar — Agent Context

## Project
SGR (Sistema de Garantia Registrada) — onchain rental guarantee infrastructure.
This repo contains Stellar/Soroban smart contracts for the NearX acceleration program.

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
