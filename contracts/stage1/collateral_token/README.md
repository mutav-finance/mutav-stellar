# `MUTAV-COL` — Stage-1 collateral token

Per-lease representative collateral record described in [`mutav-whitepaper.en.md` §5.2](../../../../mutav/docs/mutav-whitepaper.en.md). Issued as a **classic Stellar asset wrapped by the Stellar Asset Contract** (CAP-46-6, SEP-41 interface). Picking SAC over a custom Soroban SEP-41 implementation is a deliberate audit-surface decision: SAC ships the regulator-grade controls Mutav needs, so this leg has **no contract code of its own to audit**.

This directory holds issuance + control scripts, not a Soroban Rust crate. There is no `Cargo.toml` here on purpose.

## Required control surface

| Flag / mechanism | Source | Purpose |
| --- | --- | --- |
| `AUTH_REQUIRED` | classic asset flag | gate which accounts may ever hold the token |
| `AUTH_REVOCABLE` | classic asset flag | freeze a trustline |
| `AUTH_CLAWBACK_ENABLED` | CAP-0035 | securities-grade revocation |
| SEP-8 approval server | off-chain | per-transfer compliance |

## Hard invariants

- **Strictly representative.** No yield. No rebase. No redemption to a varying amount. Any of those mechanics re-classifies the token as *valor mobiliário* under CVM Parecer 40/2022 and SSE Ofício-Circular 6/2023, dragging it into the CVM offer regime. Yield mechanics belong to the Stage-2 fund-quota token under [`../../stage2/`](../../stage2/), not here.
- **Issuer account = Mutav admin (cold key).** Same authority that governs the [`reserve_vault/`](../reserve_vault/) contract.
- **Holder = agency** (off-chain KYC'd by Mutav). Exact granularity / per-unit denomination is whitepaper §11h, deferred until pinned.

## Scripts (TBD)

- `issuer-setup.sh` — set classic asset flags + home_domain for SEP-8 discovery.
- `sep8-approval-server/` — reference deny-by-default approval server.
- `mint.ts` / `clawback.ts` — issuance + revocation helpers, no key material in repo.

## Related

- Whitepaper §5.4.3 transparency recipe — agency-held collateral-token total supply via Horizon `/accounts/{issuer}` is one of the six steps in the publishable adequacy check.
- Whitepaper §11h — collateral-token semantics (granularity, holder, backing).
