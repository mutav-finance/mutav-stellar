# 2026-05-30 — Daemon PR stack (#22–#27) orphan verdict

**Status**: PROPOSED (awaiting Draau alignment via [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57))

**First ADR in `mutav-stellar/docs/architecture/decisions/`**. Convention: `YYYY-MM-DD-<topic>.md`, modelled on `mutav-app/docs/architecture/decisions/`.

## Context

Six PRs land Bun-daemon scaffolding under `src/jobs/`:

| PR | Daemon | Author | Review state | Branch |
|---|---|---|---|---|
| [#22](https://github.com/mutav-finance/mutav-stellar/pull/22) | on-ramp | @draaujpeg | CHANGES_REQUESTED | `feat/backend-onramp` |
| [#23](https://github.com/mutav-finance/mutav-stellar/pull/23) | off-ramp | @draaujpeg | CHANGES_REQUESTED | `feat/backend-offramp` |
| [#24](https://github.com/mutav-finance/mutav-stellar/pull/24) | yield-sync | @draaujpeg | REVIEW_REQUIRED | `feat/backend-yield-sync` |
| [#25](https://github.com/mutav-finance/mutav-stellar/pull/25) | heartbeat | @draaujpeg | REVIEW_REQUIRED | `feat/backend-heartbeat` |
| [#26](https://github.com/mutav-finance/mutav-stellar/pull/26) | mgmt-fee | @draaujpeg | CHANGES_REQUESTED | `feat/backend-mgmt-fee` |
| [#27](https://github.com/mutav-finance/mutav-stellar/pull/27) | ttl-watchdog | @draaujpeg | CHANGES_REQUESTED | `feat/backend-ttl-watchdog` |

[`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57) (2026-05-30 brainstorm consolidation) moves the operator runtime from this repo to **KMS-backed Convex Actions on `mutav-app`**. The Bun daemons in these 6 PRs are the wrong shape for the target. Three pieces of value in the stack are at risk if the PRs simply close:

1. **Contract-side changes** embedded in the PRs (e.g. PR #22's `receive_payment` `tx_hash` arg + `SeenTxHash` replay-guard) — these are on the audited surface and need to land here regardless of where the operator runtime runs.
2. **Audit findings** in PR-review threads — known bugs and edge cases that the future Convex Actions must respect.
3. **Some TS scaffolding** (nominal types, fee strategy, read-only query helpers, sorobanClient memoization) that might cross-apply to either the SDK or the Convex Actions.

## Options considered

**(A) Close all 6 as won't-do; reformulate as Convex Action issues.**
- Pro: clean break, no half-merged Bun daemons sitting on `main`.
- Con: contract-side fixes in the PRs (esp. PR #22 replay-guard) get dropped on the floor unless extracted.

**(B) Extract contract-only fixes from each PR as their own PRs; close the daemon scaffolding.**
- Pro: preserves the audit-relevant contract surface changes. Aligns with this repo's new scope (contract + SDK).
- Con: more PRs to open + review. PR #22 is the only one with substantial contract-side changes; the other 5 are mostly daemon-side.

**(C) Land all 6 as-is; remove later during the Convex migration.**
- Pro: zero immediate work.
- Con: ships Bun daemons that will never deploy. Pollutes `main`. Contradicts the new repo scope. Inflates the audit surface unnecessarily.

## Decision

**Recommendation: (B) for PR #22, (A) for PRs #23–#27.**

Per-PR plan:

### PR #22 — on-ramp

- **Contract-side change**: `receive_payment(imobiliaria, amount, tx_hash)` adds a `tx_hash` arg, gated by `SeenTxHash(tx_hash)` in temporary storage (~2.9-day TTL). This is a contract-layer replay guard.
- **Verdict**: extract the contract change to its own PR (`feat(contract): receive_payment replay guard via tx_hash`). Close the daemon-scaffolding portion.
- **Convex Action issue** to file against `mutav-app`: "on-ramp Convex Action — Horizon polling + receive_payment". Reference PR #22 for the cursor-race / TTL-math / decimal-truncation review findings the Action must respect.

### PR #23 — off-ramp

- No new contract-side change; uses existing `process_redemptions` + `fulfill_redemption`.
- **Verdict**: close as orphaned.
- **Convex Action issue** to file against `mutav-app`: "off-ramp Convex Workflow — weekly redemption cycle". Reference PR #23 review for the recovery-on-partial-fulfill and 24h-timeout findings — the Workflow design must close these (Convex Workflow durability solves them natively, but the design needs to spell that out).

### PR #24 — yield-sync

- No new contract-side change; reads `aum()` + `max_aum_increase_bps()`, calls `add_yield()` in a batched loop.
- **Verdict**: close as orphaned.
- **Convex Action issue**: "yield-sync Convex Action — record Etherfuse yield". Reference PR #24 for stale-cap and operator-confirmation review findings.

### PR #25 — heartbeat

- No new contract-side change; calls `extend_ttl()`.
- **Verdict**: close as orphaned.
- **Convex Action issue**: "heartbeat Convex cron — instance TTL renewal". Reference PR #25 for silent-failure surface concerns.

### PR #26 — mgmt-fee

- No new contract-side change; calls `charge_mgmt_fee()` + Stellar Classic payment.
- **Verdict**: close as orphaned.
- **Convex Action issue**: "mgmt-fee Convex Workflow — monthly charge + PIX payout". Reference PR #26 for the atomic-split bug, PIX MEMO overflow, USDC≠TESOURO 1:1 review findings — the Workflow must address all three.

### PR #27 — ttl-watchdog

- No new contract-side change; calls `extend_balance_ttl(investor)`.
- **Verdict**: close as orphaned.
- **Convex Action issue**: "ttl-watchdog Convex Action — per-investor balance TTL renewal". Reference PR #27 for cold-boot data-loss and non-atomic state-file findings — the Convex table replaces the JSON file; bootstrap-from-deployment-ledger replaces the 24h lookback default.

## TS scaffolding — what crosses over to the SDK

Some of the work in the PR stack is genuinely cross-applicable. The list below is what should be preserved (location TBD: SDK in this repo vs. mutav-app shared utility):

- **Nominal types** (`Usdc6`, `Stroops`, `StellarAccount`, `ContractId`, `EpochMs`, `LedgerSeq`) from issue #37 — belong in `src/` here, exported from the published SDK. Both this repo's tooling and the Convex Actions consume them.
- **`sorobanClient` memoization fix** — already in `src/providers/soroban/client.ts` on `main` (commit `79679ac`); no rework needed.
- **`invoke()` timeout cap** (issue #35) — belongs in the SDK if it's a useful primitive for any consumer; otherwise lives in the Convex Action layer.
- **Fee strategy** (`src/providers/soroban/fee.ts` from issue #38) — belongs in the SDK; consumers (including Convex Actions) reuse.
- **Read-only query helper** — likely belongs in the SDK; both PR #24 and PR #26 reimplemented it.

Items that **do not** cross over (Convex provides equivalents natively):
- Structured logger, retry/backoff, graceful-shutdown helpers — Convex's runtime provides these.
- Bootstrap validation, env schema validator — Convex env handling supersedes.
- Mainnet-readiness guard — moves to a Convex-side check or stays on the contract-deployment script.

## Open questions for Draau

1. **Any of the daemon TS code worth porting wholesale** to Convex Actions (vs. rewriting against Convex idioms)? My read is the contract-call wrappers are reusable via the SDK; the daemon-shaped state-machine code isn't a 1:1 port to Convex Actions.
2. **Should PR #22's contract-side extraction be authored as a new PR off `main`** or rebased from `feat/backend-onramp`? Rebase is cleaner-looking; new PR is cleaner-scoped.
3. **Convex Workflow for off-ramp and mgmt-fee** — agree this is the right durability primitive? Alternative would be Convex's built-in mutation atomicity for shorter Actions, but the off-chain-side wait (Etherfuse liquidation, PIX settlement) needs Workflow.
4. **Per-Action scoped keys vs. one shared operator key at launch** — bootstrap with shared key + rotation procedure (faster to mainnet) or hold mainnet for per-Action keys (cleaner blast-radius)?

## Sequence

1. **This ADR lands** (Status: PROPOSED) with the rest of the architecture-consolidation PR.
2. **Draau review on this PR** — adjustments to verdict per his input.
3. **Status flips to ACCEPTED** in a follow-up commit (or amendment to this PR).
4. **PR #22 contract-side extraction** opened as a new PR.
5. **PRs #22–#27 close** with comments linking to this ADR + the new Convex Action issues filed against `mutav-app`.
6. **6 issues filed against `mutav-app`** — one per Action/Workflow. Each links the original PR + this ADR.

## References

- [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57) — architecture consolidation
- [`#41`](https://github.com/mutav-finance/mutav-stellar/issues/41) — key custody + rotation runbook
- [`mutav-app#139`](https://github.com/mutav-finance/mutav-app/issues/139) — monorepo migration planning ask
- [`05-operational-layer.md`](../05-operational-layer.md) — the architecture description of the six responsibilities under their target Convex shape
- Original PRs: [#22](https://github.com/mutav-finance/mutav-stellar/pull/22), [#23](https://github.com/mutav-finance/mutav-stellar/pull/23), [#24](https://github.com/mutav-finance/mutav-stellar/pull/24), [#25](https://github.com/mutav-finance/mutav-stellar/pull/25), [#26](https://github.com/mutav-finance/mutav-stellar/pull/26), [#27](https://github.com/mutav-finance/mutav-stellar/pull/27)
