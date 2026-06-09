# Pause / Unpause Runbook

## Purpose

Use this runbook when the protocol team needs to trip the `Fund` contract
kill-switch. `set_paused(true)` is an admin-only control that stops the operator
hot path and new investor entry/exit requests while leaving investor recovery
paths available. The contract invariant in
`contracts/stage2/fund/src/lib.rs:1012-1016` says that deposits, redemptions,
yield additions, and management fees are blocked when paused, while
`cancel_redemption` and `reclaim_expired_redemption` remain callable so
investors can recover funds regardless of contract state.

## Trigger Conditions

Pause is appropriate when continued operator activity can worsen the incident:

- Confirmed or strongly suspected operator-key compromise before rotation is
  complete.
- Suspected share-inflation, NAV manipulation, or unexpected accounting drift in
  progress.
- Etherfuse or off-ramp partner outage for longer than the incident commander
  threshold while the redemption queue is growing.
- On-chain anomaly such as an unexpected `aum` drop, USDC balance mismatch, or
  event stream gap detected by the indexer.
- Regulatory order requiring temporary suspension of normal fund operations.
- Discovery of an audit-finding-class bug in deployed code, especially one that
  may require the recovery or upgrade decisions tracked in #29 and #34.

## Decision Matrix

| Situation | Preferred action | Why |
| --- | --- | --- |
| Operator key may be compromised or automation is submitting bad calls | Pause, then follow #41 for operator-key custody and rotation | Pause stops the operator hot path before more state changes land. |
| A tenant default or sinistro has happened, but admin and operator authority are healthy | Use the #19 `cover_default` runbook | The issue is economic loss coverage, not an unsafe execution path. |
| Admin key custody is compromised or planned cold-wallet rotation is due | Pause if there is live risk, then follow the admin handover runbook in #91 | Pause reduces activity during the two-step `propose_admin` / `accept_admin` ceremony. |
| Indexer alert is noisy, stale, or still below materiality | Wait and monitor | Pausing is disruptive and should not be used for unconfirmed telemetry noise. |
| Threat-model scenario points to a contract bug or trust-boundary failure | Pause and open the #46 threat-model / incident-response track | The pause buys time; it does not remediate code or process gaps. |

If in doubt during an incident, pause early when additional operator calls can
increase loss or make reconciliation harder. Prefer wait-and-monitor only when
the suspected issue is clearly off-chain, low impact, and bounded.

## Pause Procedure

1. Name an incident commander and record the trigger condition, current ledger,
   current `aum`, queue length, USDC contract balance, and latest indexed event.
2. Confirm current pause state by reading `paused() -> bool` at
   `contracts/stage2/fund/src/lib.rs:1131`. The SDK does not currently expose a
   dedicated read helper, so use the existing Soroban RPC read path or contract
   CLI until that gap is closed.
3. Have the protocol-team admin open the hardware-wallet signing surface in
   `mutav-app/apps/admin/`. The trust model documents this admin boundary in
   `docs/architecture/02-actors-and-trust.md`.
4. Compose a Soroban operation for `set_paused(true)` at
   `contracts/stage2/fund/src/lib.rs:1016`. The SDK currently has no
   `buildSetPausedOp` helper in `src/providers/soroban/fund.ts`; document the
   raw shape as `new Contract(contractId).call("set_paused",
   nativeToScVal(true))` until the builder is added under the SDK workstream.
5. Present the XDR to the admin signer, verify the contract ID and boolean
   argument in the wallet UI, sign, prepare, and submit.
6. Verify the transaction result includes the `set_paus(true)` event emitted at
   `contracts/stage2/fund/src/lib.rs:1019`.
7. Re-read `paused()` and confirm it returns `true`.
8. Freeze operator automation in `mutav-app` so Convex Actions do not keep
   retrying blocked calls.
9. Record a drift signal: `sweep_usdc` currently has no `require_not_paused`
   guard at `contracts/stage2/fund/src/lib.rs:887-889`, so responders must
   explicitly disable or supervise any sweep pathway during the pause.

The current `require_not_paused` call sites block these 9 entrypoints:

| Entrypoint | Evidence |
| --- | --- |
| `receive_payment` | `contracts/stage2/fund/src/lib.rs:444-446` |
| `deposit_investor` | `contracts/stage2/fund/src/lib.rs:503-505` |
| `request_redemption` | `contracts/stage2/fund/src/lib.rs:535-537` |
| `process_redemptions` | `contracts/stage2/fund/src/lib.rs:599-601` |
| `fulfill_redemption` | `contracts/stage2/fund/src/lib.rs:715-717` |
| `add_tenant_fee` | `contracts/stage2/fund/src/lib.rs:786-788` |
| `add_yield` | `contracts/stage2/fund/src/lib.rs:806-808` |
| `charge_mgmt_fee` | `contracts/stage2/fund/src/lib.rs:827-829` |
| `record_offchain_payout` | `contracts/stage2/fund/src/lib.rs:852-854` |

## While Paused

- Monitor the investor queue length and confirm it stops growing because
  `request_redemption` is blocked.
- Monitor existing `ReadyRedemption` deadlines and the
  `ready_redemption_deadline` view. Investors can still use
  `reclaim_expired_redemption` after the fulfill window.
- Monitor pending redemptions that may need `cancel_redemption`; the pause must
  not block investor recovery.
- Keep an incident communications log with investor notice, partner notice,
  regulator notice when applicable, and internal decision timestamps.
- Reconcile indexed events against Soroban RPC and the latest operator logs. Link
  the reconciliation evidence to #44 once observability docs land.
- Keep all operator daemons or Convex Actions stopped unless the incident
  commander explicitly approves a read-only action.

## Unpause Checklist

Do not call `set_paused(false)` until all preconditions are true:

- Root cause is documented, remediated, and reviewed by the incident commander.
- Operator key custody is healthy, rotated, or otherwise cleared through #41.
- If an admin handover was required, the #91 runbook is complete and the current
  `admin()` view returns the intended cold wallet.
- Indexer reconciliation passes against RPC, contract events, and off-chain
  operator logs.
- No regulatory, partner, or investor communication hold remains open.
- Any `cover_default` action required by #19 is complete or explicitly deferred.
- `sweep_usdc` has been reviewed for reserve safety before automation resumes.

To unpause:

1. Compose `set_paused(false)` through the same admin hardware-wallet ceremony.
2. Verify the transaction emits `set_paus(false)`.
3. Re-read `paused()` and confirm it returns `false`.
4. Resume only the operator Actions needed for the next business cycle.
5. Run one post-unpause smoke read for `aum`, `queue_len`, and a representative
   investor state before allowing normal automation cadence.
6. Publish the all-clear communication and archive the incident log.

## Escalation

Escalate beyond pause/unpause when:

- AUM is wrong or the fund needs economic coverage: use #19 `cover_default`.
- Admin or operator custody is compromised: use #91 for admin handover and #41
  for operator-key custody.
- The defect is in deployed code: open the #46 threat-model / audit-prep track
  and the relevant contract follow-up, such as #29 or #34.
- The pause blocks mainnet readiness: update #40 with the incident status and
  keep the readiness gate closed until this runbook's unpause checklist passes.
- A sibling runbook is a better fit: #87, #88, #89, #91, and #92 cover adjacent
  runbook families from the same operations track.
- Investors cannot recover through `cancel_redemption` or
  `reclaim_expired_redemption`: treat this as a severity-1 protocol incident.

## Verification Tests

Pause semantics are covered in `contracts/stage2/fund/src/lib.rs:2799-2868`:

- `pause_blocks_deposit` confirms a paused contract rejects a guarded call.
- `pause_does_not_block_cancel_redemption` confirms investors can cancel a
  pending redemption while paused.
- `pause_does_not_block_reclaim_expired_redemption` confirms investors can
  reclaim an expired ready redemption while paused.
- `unpause_restores_operations` confirms `set_paused(false)` restores normal
  operation.

When this runbook changes, verify the line anchors still match the current
contract and update the blocked-entrypoint table if `require_not_paused` moves.
