<!-- BEGIN:soroban-agent-rules -->

# Soroban Rust contracts in this repo

Run `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings` before pushing. CI runs both and will fail PRs that drift.

Storage tiers in Soroban are tier-distinct: `instance`, `persistent`, `temporary`. TTL semantics differ — `temporary` entries vanish at expiry; `persistent` entries can be revived via `extend_ttl`. Check `contracts/stage2/fund/src/lib.rs:33` for the active storage key catalog before adding new ones.

Contracts are split by whitepaper stage. `contracts/stage1/` holds the pilot-stage surface (reserve vault + SAC-wrapped collateral token; see `contracts/stage1/README.md`). `contracts/stage2/` holds the outside-investor fund vault. The contracts are the audited surface. Match existing conventions; new patterns need a reason.

<!-- END:soroban-agent-rules -->

<!-- BEGIN:bun-typescript-agent-rules -->

# TypeScript SDK via Bun

Use `bun install --frozen-lockfile` for reproducible builds. Run `bun run typecheck` before pushing. The TS SDK lives in `src/` and is published as `@mutav-finance/mutav-stellar` for consumption by [`mutav-app`](https://github.com/mutav-finance/mutav-app). The SDK is read-oriented: it composes chain reads and produces transaction XDRs that consumers sign — it does not hold any keys. Operator-authority runtime lives on `mutav-app` (KMS-backed Convex Actions); do not add daemon scaffolding here.

<!-- END:bun-typescript-agent-rules -->