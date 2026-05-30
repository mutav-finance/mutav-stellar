# 06 — Canonical flows

Five operational flows + two maintenance flows. Each diagram shows the actors that participate (contract, operator runtime, investor, etc.) and the order of events. Arrows are call/event boundaries.

> **Snapshot caveat**: flows described as of `main` at commit `90e1185`. After the [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57) consolidation (2026-05-30), the actor labelled "operator" in each flow is a **Convex Action on `mutav-app`** signing via KMS — see [`05-operational-layer.md`](./05-operational-layer.md). The flow logic, contract calls, and ordering are unchanged. PR-specific annotations (e.g. PR #22's replay-guard) refer to contract-side changes embedded in those PRs that may still land independently; see [`decisions/2026-05-30-daemon-prs-orphan-verdict.md`](./decisions/2026-05-30-daemon-prs-orphan-verdict.md) for the orphan disposition.

## Flow 1 — Partner payment ingress (on-ramp)

A partner agency pays the monthly guarantee fee in USDC to the operator address; the on-ramp Convex Action detects it and records it on-chain.

```mermaid
sequenceDiagram
    autonumber
    participant P as Partner imobiliária
    participant H as Horizon
    participant A as on-ramp Convex Action
    participant F as Fund contract
    participant CW as classic_wallet (Etherfuse)

    P->>H: Stellar payment (USDC) → operator
    A->>H: poll payments (cursor)
    H-->>A: payment record + tx_hash
    A->>F: receive_payment(imobiliaria, amount, tx_hash)
    F->>F: assert whitelisted
    Note over F: assert tx_hash not seen (SeenTxHash, ~2.9d TTL)
    F->>F: split 20%/80% → protocol + AUM
    F->>CW: USDC transfer (AUM portion)
    F-->>A: success (event: rcv_pay)
    A->>A: saveCursor (advance after on-chain success)
```

**Contract-side**: the on-chain replay guard (PR #22's `tx_hash` arg + `SeenTxHash(tx_hash)` in temporary storage) is part of the contract surface and should land independently of the Convex-Action move — see the orphan verdict. **Atomicity gap from the Bun design** (cursor advancing before on-chain success): resolved by Convex's atomic mutation semantics — the Action's mutation that bumps the cursor commits only after the contract call returns.

## Flow 2 — Investor deposit

```mermaid
sequenceDiagram
    autonumber
    participant I as Investor
    participant F as Fund contract
    participant CW as classic_wallet (Etherfuse)

    I->>F: deposit_investor(investor, amount_usdc)
    F->>F: require_auth(investor)
    F->>F: require_not_paused
    F->>F: calc_mint(amount × supply / aum)
    F->>I: pull USDC (token::transfer)
    F->>CW: forward USDC
    F->>I: mint MUTAV (balance += mutav_minted)
    F->>F: aum += amount; supply += mutav
    F-->>I: event: deposit
```

**NAV**: locked at this call's `aum/supply`. **Pause**: blocks deposit; investors can still cancel/reclaim existing redemptions.

## Flow 3 — Yield recording

```mermaid
sequenceDiagram
    autonumber
    participant E as Etherfuse (API)
    participant A as yield-sync Convex Action
    participant F as Fund contract

    E-->>A: yield amount (off-chain)
    A->>F: aum() + max_aum_increase_bps()
    F-->>A: aum, max_bps
    A->>A: maxPerCall = aum × max_bps / 10_000
    loop while remaining > 0
        A->>F: add_yield(min(remaining, maxPerCall))
        F->>F: aum += batch
        F-->>A: event: yield_in
    end
```

**Cap**: `max_aum_increase_bps` per call. **Gap**: no per-period rolling cap → operator-compromise inflates NAV (issue #31).

## Flow 4 — Redemption (request → process → fulfill)

The most complex flow because it spans on-chain queue mechanics + off-chain liquidation + per-investor finalization.

```mermaid
sequenceDiagram
    autonumber
    participant I as Investor
    participant W as off-ramp Convex Workflow
    participant E as Etherfuse
    participant F as Fund contract

    Note over I: Day 0 — request
    I->>F: request_redemption(investor, mutav_amount)
    F->>F: lock MUTAV; append to RedemptionQueue
    F-->>I: event: req_rdmpt

    Note over W: Week N — process (step 1 of Workflow)
    W->>F: process_redemptions()
    F->>F: snapshot NAV; cap by weekly_exit
    F->>F: for each fitting: burn MUTAV, write ReadyRedemption(deadline)
    F-->>W: total_usdc + rdy_rdmpt events per investor
    W->>W: persist in-flight ready set (Workflow checkpoint)

    Note over W,E: Off-chain liquidation (step 2)
    W->>E: liquidate(total_usdc)
    E-->>W: USDC on operator address (wait, up to 24h)

    Note over W: Deposit + fulfill (step 3)
    W->>F: token transfer (USDC into contract)
    loop per investor (resumable from checkpoint)
        W->>F: fulfill_redemption(investor)
        F->>I: USDC transfer (gross - redemption_fee)
        F-->>W: event: fulfill
    end

    Note over I: Escape hatch
    I->>F: reclaim_expired_redemption(investor)
    F->>I: restore MUTAV (if deadline passed without fulfillment)
```

**Gap resolved by Convex Workflow durability** (previously a Bun-daemon gap from PR #23 review): if the Workflow crashes between `process_redemptions` and the per-investor `fulfill_redemption` loop, it resumes from the last persisted checkpoint. Investors are still racing the on-chain `deadline`, but the Workflow does not lose track of who's pending — the `reclaim_expired_redemption` escape hatch remains the contract-side safety net.

## Flow 5 — Monthly mgmt fee

```mermaid
sequenceDiagram
    autonumber
    participant W as mgmt-fee Convex Workflow
    participant F as Fund contract
    participant H as Horizon (Classic)
    participant E as Etherfuse → PIX

    W->>F: aum() + mgmt_fee_bps()
    F-->>W: aum, fee_bps
    W->>F: charge_mgmt_fee()
    F->>F: assert now ≥ last_fee + 30d
    F->>F: aum -= aum × fee_bps / 10_000
    F-->>W: event: mgmt_fee
    W->>W: persist on-chain checkpoint (step 1 done)
    W->>H: Classic payment (TESOURO, MEMO=PIX key) → Etherfuse
    H-->>W: tx success/fail (Workflow retries on transient failure)
    E->>E: settle PIX to recipient bank
```

**Atomic-split bug from the Bun-daemon design**: on-chain `charge_mgmt_fee` runs before the Classic payment; if Classic failed, AUM was debited but no fee shipped, and the 30-day guard blocked retry. **Resolved by Convex Workflow**: the on-chain debit and the off-chain Classic submission are two steps of the same Workflow; the Workflow persists between them and retries the off-chain submission on transient failure. A permanent off-chain failure (e.g. malformed PIX MEMO) becomes a Workflow-level error to surface to ops rather than silent AUM-debit-with-no-payout.

## Maintenance flow A — Contract instance TTL (heartbeat)

```mermaid
sequenceDiagram
    autonumber
    participant H as heartbeat Convex cron
    participant F as Fund contract

    loop every 25 days
        H->>F: extend_ttl()
        F->>F: bump instance storage TTL
        F-->>H: event (impl. via Soroban host)
    end
```

~30-day instance lease + 5-day safety margin = 25-day renewal cadence.

## Maintenance flow B — Investor balance TTL (ttl-watchdog)

```mermaid
sequenceDiagram
    autonumber
    participant W as ttl-watchdog Convex cron
    participant F as Fund contract
    participant T as Convex investor table

    Note over W: Discovery (one-shot on first run, then incremental)
    W->>F: getEvents(deposit topic, fromLedger)
    F-->>W: deposit events (per investor)
    W->>T: upsert discovered investors + ledger cursor

    Note over W: Renewal
    loop every 25 days per investor
        W->>F: extend_balance_ttl(investor)
        F->>F: bump persistent Balance(investor) TTL
        W->>T: lastExtended[investor] = now
    end
```

**Gap from the Bun-daemon design** (PR #27 review): the prior cold-boot defaulted to a 24h lookback, silently dropping investors who deposited before that window. **Resolution under Convex**: the discovery cursor lives in a Convex table, not a JSON file on a single host, so there is no per-host cold boot. The first-ever run must still seed from ledger zero (or from the contract's deployment ledger); add an explicit bootstrap-from-deployment-ledger step in the Action rather than a lookback default.

## Cross-flow invariants

- All flows respect `paused` semantics (see [03-contract.md](./03-contract.md) for the matrix of what pause blocks).
- All flows that change AUM also emit a topic-tagged Soroban event. The future indexer (#44) consumes these for the audit log.
- The redemption queue is FIFO ordering of requests, but a second `request_redemption` from the same investor accumulates onto their existing entry (does NOT enqueue a new slot). Surfaces as a fairness consideration in issue #34.
