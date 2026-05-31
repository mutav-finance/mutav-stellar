# 2026-05-30 — Daemon PR stack (#22–#27) orphan verdict

**Status**: ACCEPTED (recorded 2026-05-31; Draau is not reviewing the doc-consolidation, so decisions were made directly by @jubscodes)

**First ADR in `mutav-stellar/docs/architecture/decisions/`**. Convention: `YYYY-MM-DD-<topic>.md`, modelled on `mutav-app/docs/architecture/decisions/`.

## Decisions recorded 2026-05-31

Acceptance changed the scope from what the original PROPOSED draft envisioned: as we walked through each PR's actual contents, it became clear that the helpers I'd initially labelled as "SDK-bound" (`invoke.ts`, `token.ts`, `classic.ts`) are operator-runtime helpers (they sign and submit transactions) and do **not** match the merged SDK scope ("Read-oriented SDK; composes chain reads and produces transaction XDRs that consumers sign — it does not hold any keys"). The same indictment applies to code already on `main`: `src/core/wallet.ts` (loads operator keypair) and `src/providers/soroban/fund.ts` (8 sign-and-submit wrappers).

Q1–Q4 answers + the corrected scope:

1. **TS porting strategy** — *helpers reviewed PR-by-PR*. Result: only `src/core/units.ts` (6 LOC, `parseStellarUsdc`) is genuine SDK material. Everything else either (a) is operator-runtime code that moves to `mutav-app` or (b) is already on `main` in a shape that needs restructuring.
2. **PR #22 contract-side extraction** — *new PR off `main`*. Cleanest scope; reviewers see only the audited-surface change. Loses the daemon-side review history (acceptable).
3. **Convex Workflow for off-ramp + mgmt-fee** — *yes, both*. Convex Workflow's persisted-step semantics resolve the atomic-split bug in mgmt-fee and the partial-fulfill recovery gap in off-ramp.
4. **Operator key strategy at mainnet launch** — *hybrid*. Shared key for low-risk renewal crons (heartbeat, ttl-watchdog); per-Action scoped keys for AUM-touching Actions (on-ramp, off-ramp, yield-sync, mgmt-fee). Compromise between blast-radius isolation and ship speed.

Implication: the original "extract contract change from #22, file 5 Convex Action issues against `mutav-app`" plan expands to also include two SDK-scope-down PRs against `mutav-stellar` (drop the signing layer from `fund.ts`; remove `wallet.ts`).

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

**Accepted: (B) for PR #22, (A) for PRs #23–#27, plus two SDK scope-down PRs against `main` to fix code already merged that doesn't match the new SDK scope.**

Per-PR plan:

### PR #22 — on-ramp

- **Contract-side change**: `receive_payment(imobiliaria, amount, tx_hash)` adds a `tx_hash` arg, gated by `SeenTxHash(tx_hash)` in temporary storage (~2.9-day TTL). This is a contract-layer replay guard. **Audit-relevant.**
- **Verdict**: extract the contract change to its own PR against `main` (`feat(contract): receive_payment replay guard via tx_hash`). Add a new test `receive_payment_rejects_duplicate_tx_hash` that PR #22 did not include — the audit will want explicit coverage of the guard firing. Close the daemon-scaffolding portion of PR #22.
- **Convex Action issue** to file against `mutav-app`: "on-ramp Convex Action — Horizon polling + receive_payment". Reference PR #22 for the cursor-race / TTL-math / decimal-truncation review findings the Action must respect.

### PR #23 — off-ramp

- Adds 3 SDK-shaped files (`invoke.ts`, `token.ts`, `units.ts`) + `off-ramp.ts` daemon. On review, only `units.ts` (6 LOC, `parseStellarUsdc`) is true SDK material — the other two are operator-runtime helpers that sign+submit.
- **Verdict**: close as orphaned.
- **Cherry-pick**: `src/core/units.ts` can be reapplied as part of the SDK restructure PR (PR B below). The signing helpers (`invoke.ts`, `token.ts`) port to `mutav-app` as Convex Action utilities, not to the SDK.
- **Convex Action issue**: "off-ramp Convex Workflow — weekly redemption cycle". Per Q3, uses Convex Workflow. Reference PR #23 review for the recovery-on-partial-fulfill and 24h-timeout findings — the Workflow design closes both natively.

### PR #24 — yield-sync

- Adds `yield-sync.ts` daemon + `fund.ts` extension (`add_yield` wrapper) + `client.ts` memoization fix. The `client.ts` memoization is already on `main` via commit `79679ac` (PR #11) — no port needed.
- **Verdict**: close as orphaned.
- **Convex Action issue**: "yield-sync Convex Action — record Etherfuse yield". Reference PR #24 for stale-cap and operator-confirmation review findings.

### PR #25 — heartbeat

- Adds only `heartbeat.ts` (55 LOC Bun daemon with a 25-day `Bun.sleep` loop). No SDK material.
- **Verdict**: close as orphaned.
- **Convex Action issue**: "heartbeat Convex cron — instance TTL renewal". Reference PR #25 for silent-failure surface concerns.

### PR #26 — mgmt-fee

- Adds `mgmt-fee.ts` daemon + `classic.ts` (`sendClassicPayment` signs and submits Classic XDRs) + `fund.ts` extension. Same indictment as #23 — `classic.ts` is operator-runtime, not SDK.
- **Verdict**: close as orphaned.
- **Cherry-pick**: none for the SDK. `classic.ts` ports to `mutav-app` with the PIX MEMO length validation fix that PR #26 review flagged (currently caller passes raw string and PIX UUID/email keys overflow silently).
- **Convex Action issue**: "mgmt-fee Convex Workflow — monthly charge + PIX payout". Per Q3, uses Convex Workflow. The Workflow design closes the atomic-split bug, PIX MEMO overflow, and the USDC≠TESOURO 1:1 finding.

### PR #27 — ttl-watchdog

- Adds only `ttl-watchdog.ts` (131 LOC Bun daemon with a JSON state file on disk). No SDK material.
- **Verdict**: close as orphaned.
- **Convex Action issue**: "ttl-watchdog Convex Action — per-investor balance TTL renewal". The Convex table replaces the JSON file; bootstrap-from-deployment-ledger replaces the 24h-lookback default that PR #27 review flagged.

### PR B (new) — SDK scope-down: drop signing layer from `src/providers/soroban/fund.ts`

`fund.ts` on `main` exports `invoke()` + 8 sign-and-submit wrappers (`receivePayment`, `addYield`, `chargeMgmtFee`, `processRedemptions`, `fulfillRedemption`, `recordOffchainPayout`, `extendTtl`, `extendBalanceTtl`) — each takes a `Keypair` and submits. This shape is operator-runtime, not SDK.

- **Verdict**: rewrite to expose `xdr.Operation` builders only (e.g. `buildReceivePaymentOp(contractId, imobiliaria, amount, txHash): xdr.Operation`). Drop `invoke()`. Drop the `Keypair` parameter.
- **Consumer impact**: nothing imports these wrappers from `mutav-app` yet — the Convex Action layer that will consume them does not exist. Safe rewrite window.
- **Test plan**: `bun run typecheck` still passes; package exports continue to expose `./soroban/fund` with the new signatures.

### PR C (new) — SDK scope-down: remove `src/core/wallet.ts`

`wallet.ts` on `main` exports `loadOperatorKeypair` (reads `OPERATOR_SECRET` from env) and `loadFundContractId`. The first is operator-runtime concern (this repo's deployment no longer holds operator keys per the merged scope); the second can live wherever the SDK consumer needs it.

- **Verdict**: delete `src/core/wallet.ts`. Remove `./core/wallet` from `package.json` exports. `loadFundContractId` is one line — consumers re-derive trivially.
- **Consumer impact**: no current consumer.

## SDK scaffolding — final categorization

After the per-PR walkthrough, what crosses over into the SDK (`src/`):

- **`src/core/units.ts`** (6 LOC) — `parseStellarUsdc()` converts Stellar Classic's 7-decimal asset string to bigint 6-decimal contract units. Pure conversion. Cherry-pick from PR #23 into PR B.
- **`src/core/network.ts`** — already on `main`. Network config (testnet/mainnet) with `resolveNetwork()`. Stays.
- **`src/providers/soroban/client.ts`** — already on `main` with memoization. RPC `Server` factory. Stays.
- **`src/providers/soroban/fund.ts`** — already on `main` in sign-and-submit shape. **Restructure to XDR-builders only** (PR B above).

What moves to `mutav-app` (Convex Action layer):

- `invoke.ts`, `token.ts` from PR #23 — rewrite as Convex Action utilities that fetch the operator key from KMS (per Q4's hybrid strategy: scoped key for the AUM-touching Actions, shared key for the renewal crons).
- `classic.ts` from PR #26 — rewrite with PIX MEMO length validation.
- All 6 `src/jobs/*.ts` daemon files — rewrite as Convex Actions (heartbeat, ttl-watchdog, yield-sync, on-ramp) or Workflows (off-ramp, mgmt-fee per Q3).

What gets removed from `mutav-stellar` entirely:

- `src/core/wallet.ts` — operator-key loading. Removed in PR C.

## Sequence

1. ~~ADR lands as PROPOSED~~ — done (commit `e7f837f`, merged in PR #58 on 2026-05-30).
2. ~~Draau review~~ — Draau is not reviewing the doc-consolidation. Decisions made directly by @jubscodes on 2026-05-31.
3. ~~Status flips to ACCEPTED~~ — this amendment.
4. **PR A**: extract PR #22's contract-side replay guard as a new PR off `main` (`feat(contract): receive_payment replay guard via tx_hash`). Adds the missing `receive_payment_rejects_duplicate_tx_hash` test.
5. **PR B**: SDK scope-down — rewrite `src/providers/soroban/fund.ts` as XDR-builders (no signing); cherry-pick `src/core/units.ts` from PR #23.
6. **PR C**: SDK scope-down — remove `src/core/wallet.ts`; update `package.json` exports.
7. **PRs #22–#27 close** with comments linking this ADR + the new Convex Action issues filed against `mutav-app`.
8. **6 issues filed against `mutav-app`** — one per Action/Workflow. Each links the original PR + this ADR.

PRs A/B/C can land in any order; they touch independent file sets. PR closures and `mutav-app` issues happen after PRs A/B/C are open.

## References

- [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57) — architecture consolidation
- [`#41`](https://github.com/mutav-finance/mutav-stellar/issues/41) — key custody + rotation runbook
- [`mutav-app#139`](https://github.com/mutav-finance/mutav-app/issues/139) — monorepo migration planning ask
- [`05-operational-layer.md`](../05-operational-layer.md) — the architecture description of the six responsibilities under their target Convex shape
- Original PRs: [#22](https://github.com/mutav-finance/mutav-stellar/pull/22), [#23](https://github.com/mutav-finance/mutav-stellar/pull/23), [#24](https://github.com/mutav-finance/mutav-stellar/pull/24), [#25](https://github.com/mutav-finance/mutav-stellar/pull/25), [#26](https://github.com/mutav-finance/mutav-stellar/pull/26), [#27](https://github.com/mutav-finance/mutav-stellar/pull/27)
