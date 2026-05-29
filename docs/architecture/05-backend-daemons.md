# 05 — Backend daemons

Six long-lived processes wrap the contract for day-to-day operations. All are in `src/jobs/*.ts` per the PR #22–#27 stack; the shared helpers live in `src/core/*` and `src/providers/*`. Each daemon owns one operational responsibility, runs on its own schedule, and (currently) shares one operator keypair.

> **Why they live here, not on `mutav-app` or `mutav-fund`**: every daemon holds operator authority and signs on-chain transactions. The sibling repos are UI surfaces (with their own backends in the case of `mutav-app`/Convex) but neither holds operator keys. Daemons live here because operator-key custody belongs with the contracts they authorize.
>
> **They are not "audited" in the same sense the Rust contract is.** The contract is the audit-gated surface; daemons are operator-authority software that changes weekly and needs its own change-control regime (release tags, separate review bar). The 6 PRs landing here, with mixed audit verdicts, are the live evidence of that gap — they have not been through the same rigor the contract will get pre-mainnet.

## The six daemons

| # | Daemon | Purpose | Cadence | PR | Audit status |
|---|---|---|---|---|---|
| 1 | **on-ramp** | Poll Horizon for partner USDC payments → call `receive_payment` | 30s poll | #22 | CHANGES_REQUESTED (cursor race, TTL math, decimal truncation) |
| 2 | **off-ramp** | Weekly redemption cycle: `process_redemptions` → Etherfuse liquidation → `fulfill_redemption` per investor | Weekly | #23 | CHANGES_REQUESTED (no recovery on partial fulfill; 24h timeout strands state) |
| 3 | **yield-sync** | Record Etherfuse yield onto AUM via `add_yield`, batched against per-call cap | On-demand CLI | #24 | COMMENT (stale cap; no operator confirmation) |
| 4 | **mgmt-fee** | Monthly: `charge_mgmt_fee` on-chain + Classic-asset PIX payout | Monthly | #26 | CHANGES_REQUESTED (atomic split bug, PIX MEMO overflow, USDC≠TESOURO 1:1) |
| 5 | **heartbeat** | Renew contract instance TTL every 25 days | Every 25d | #25 | COMMENT (silent-failure surface; merge with follow-ups) |
| 6 | **ttl-watchdog** | Renew each investor's balance TTL via `extend_balance_ttl` | Every 25d/investor | #27 | CHANGES_REQUESTED (cold-boot data loss; non-atomic state file) |

## State ownership

| Daemon | On-chain state mutated | Off-chain state owned |
|---|---|---|
| on-ramp | `receive_payment` → AUM + protocol cut. **(PR #22)** also writes `SeenTxHash` for replay protection | `.on-ramp-cursor` file (Horizon paging token) |
| off-ramp | `process_redemptions` (queue → ready) + `fulfill_redemption` per investor | none persistent today — **bug**; should track in-flight ready set across crashes (issue #38) |
| yield-sync | `add_yield` (AUM↑) | none |
| mgmt-fee | `charge_mgmt_fee` (AUM↓) + Classic payment submission | none — and that's the atomic-split bug (#26 review) |
| heartbeat | `extend_ttl()` (instance) | none |
| ttl-watchdog | `extend_balance_ttl(investor)` per investor | `data/ttl-watchdog.json` — `{ ledgerCursor, investors[], lastExtended{} }` |

## Coordination model

There is no daemon-to-daemon coordination. Each runs independently against the same contract. The contract's per-function invariants (replay-guard, 30-day mgmt-fee interval, weekly exit cap, TTL leases) are the only synchronization.

**This works** for daemons with disjoint state mutations. **It fails** for the daemons that span an on-chain/off-chain transaction boundary (off-ramp, mgmt-fee) — those need persistent in-flight state to recover from crashes.

## Foundation layer (current vs needed)

What exists on `main` today (`src/core/*`, `src/providers/*`):

```
src/core/
  network.ts       — testnet/mainnet config (hardcoded RPC URLs)
  wallet.ts        — loadOperatorKeypair, loadFundContractId (raw env, no validation)
src/providers/soroban/
  client.ts        — rpc.Server factory (no memoization on main)
  fund.ts          — invoke() + 8 wrappers for contract calls
```

What every daemon needs but doesn't have (issue #38):

- **Structured logger** (`src/core/log.ts`)
- **Retry/backoff helper** (`src/core/retry.ts`)
- **Graceful-shutdown helper** (`src/core/shutdown.ts`)
- **Mainnet-readiness guard** (`src/core/guards.ts`)
- **Env schema validator** (`src/core/env.ts`)
- **Bootstrap validation** (`src/core/bootstrap.ts`) — chain identity, contract format, on-chain auth check — issue #36
- **Nominal types** (`src/core/types.ts`) — `Usdc6`, `Stroops`, `StellarAccount`, `ContractId`, `EpochMs`, `LedgerSeq` — issue #37
- **Fee strategy** (`src/providers/soroban/fee.ts`) — surge pricing, fee bumps
- **Read-only query helper** — currently duplicated in PRs #24 and #26

Plus three immediate fixes to existing files:
- `sorobanClient` memoization (PR #23/#24 each ship one — should be on main)
- `invoke()` timeout cap (issue #35 — currently polls `NOT_FOUND` forever)
- Better `errorResult` parsing (currently JSON-stringified into one log line)

## Process model (assumed)

| Concern | Today | Target |
|---|---|---|
| Process count | 6 separate Bun processes | Same (one job per process — easier to monitor/restart) |
| Key isolation | All 6 share `OPERATOR_SECRET` | Per-daemon scoped keys or per-daemon HSM access (issue #41) |
| Host | Undocumented | TBD; could be one box or one container each |
| Restart policy | `Bun.sleep` loop with ad-hoc try/catch | systemd / k8s with explicit restart policy + health probes (issue #44) |
| Observability | `console.log("[name] ...")` | Structured JSON + `/healthz` + `/metrics` per daemon (issue #44) |

## Known gaps

- All issues #35–#38 (foundation) and #41 (key custody), #44 (observability).
- The six daemons are the most-impacted by the foundation modules in #38 — every one of them currently reimplements a piece of what should be shared.
- PR #23 and PR #26 specifically need a persistence layer for in-flight on-chain/off-chain split state — that's the missing pattern.
