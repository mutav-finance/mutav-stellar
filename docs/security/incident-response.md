# Vulnerability Disclosure Incident Response

This runbook defines how maintainers respond after a security report lands for
the audited `mutav-stellar` surface: the Soroban `Fund` contract and the
read-oriented `@mutav-finance/mutav-stellar` SDK.

Reports about dependent applications, Convex Action internals, Stellar core, RPC
providers, wallets, or third-party dependencies should be routed from
`SECURITY.md` to the owning project. Issues that cross the boundary into
`mutav-app` still need coordination here when they affect contract behavior or
SDK consumers.

## Intake

GitHub private vulnerability reporting is the canonical channel. If that channel
is unavailable, use the maintainer contact listed in `SECURITY.md`.

The first responder should:

1. Acknowledge the report within 5 business days.
2. Create a private tracking thread with the reporter, impacted maintainers, and
   any required `mutav-app` counterpart.
3. Record affected component, reproduction status, suspected severity, reporter
   credit preference, and whether the issue appears exploited in the wild.
4. Avoid public issues, public branches, or public CI logs until containment and
   disclosure timing are agreed.

## Severity Rubric

Use the highest applicable severity. When uncertain, escalate one tier until the
blast radius is proven smaller.

| Severity | Contract or SDK blast radius |
|---|---|
| Critical | Drains or freezes AUM; mints unbacked `MUTAV`; corrupts `PendingRedemption`, `ReadyRedemption`, or `RedemptionQueue`; bypasses admin/operator auth; breaks the `SeenTxHash` replay guard; lets an attacker call an AUM-debiting entrypoint without the intended authority. |
| High | Privilege escalation requiring non-default state; denial of redemption flow; NAV/AUM accounting drift above configured tolerance; SDK XDR builders produce auth-mismatched or value-mismatched transactions that consumers could submit. |
| Medium | Griefing, gas or footprint denial of service, storage TTL exhaustion, unsafe SDK type surfaces that require caller misuse, or disclosure of non-secret operational metadata. |
| Low | Documentation issues, non-exploitable assertions, confusing examples, or defense-in-depth hardening with no demonstrated exploit path. |

Primary state and authority anchors:

- `contracts/fund/src/lib.rs:33-75` — storage catalog including `Paused`,
  `PendingAdmin`, `SeenTxHash`, `WeeklyExitUsed`, and redemption queues.
- `contracts/fund/src/lib.rs:309-316` — `require_not_paused` gate.
- `contracts/fund/src/lib.rs:444-887` — operator and user flows touching
  partner payments, deposits, redemptions, yield, fees, off-chain payouts, TTL,
  and USDC sweeps.
- `contracts/fund/src/lib.rs:907-1042` — admin containment and authority flows:
  `cover_default`, partner whitelist, wallet/operator setters, parameter
  setters, `set_paused`, `propose_admin`, and `accept_admin`.
- `src/` SDK code is read-oriented and does not own signing keys; SDK incidents
  are usually transaction-construction, RPC consistency, dependency, or consumer
  integration risks.

## Containment Levers

### Pause contract activity

Use `set_paused(true)` when a report suggests active AUM loss, redemption queue
corruption, partner-payment replay, or an unknown critical class.

- Authority: admin hardware-wallet flow on `mutav-app/apps/admin/`.
- Contract anchor: `set_paused` writes the instance `Paused` flag.
- Effect: user-facing and operator flows protected by `require_not_paused` halt.
- Caveat: admin setters and recovery operations do not all behave identically
  under pause; confirm the target entrypoint before relying on pause as the only
  containment.

### Rotate operator authority

Use `set_operator(new_operator)` for operator-key compromise, KMS credential
compromise, suspicious signing activity, or a Convex Action boundary failure.

- Authority: admin hardware-wallet flow.
- Follow the operator-key rotation design in
  `docs/specs/2026-05-31-operator-key-runbook-design.md`.
- Coordinate with `mutav-app` so Convex Actions reload the new KMS key handle
  before unpausing.

### Transfer or replace admin authority

Use `propose_admin(new_admin)` followed by `accept_admin()` for admin-key
rotation or suspected admin custody compromise.

- Authority: current admin proposes; pending admin self-authenticates to accept.
- Treat suspected admin compromise as Critical unless the compromised key is
  provably unable to sign.
- If the attacker may still sign as admin, prioritize freezing downstream
  operational surfaces and coordinating with reviewers before publishing
  details.

### Patch or yank SDK releases

For SDK issues:

- Patch privately first, then publish a fixed version.
- Yank or deprecate affected npm versions when continued installation is unsafe.
- Coordinate `mutav-app` consumer upgrades before public disclosure when the SDK
  bug can produce unsafe XDR or stale state display.

## Patch Staging

Contract patches:

1. Reproduce privately with the smallest failing test or simulation.
2. Patch on a private branch or private fork.
3. Run the normal gates before release: `cargo fmt`, `cargo clippy -- -D warnings`,
   and the contract test suite. Do not use `--no-verify` to bypass checks during
   incident response.
4. Request audit-firm or reviewer sign-off for Critical and High issues.
5. Deploy using the admin-signed contract upgrade path or migration plan
   appropriate for the affected release.

SDK patches:

1. Reproduce with a focused unit or integration test.
2. Patch privately and run typecheck/test gates, including `bun run typecheck`
   where applicable.
3. Publish a patched version and coordinate downstream consumer upgrades.
4. Prepare public advisory text with affected versions and mitigation steps.

## Cross-Repo Coordination

The deployment boundary is fixed by
`docs/architecture/decisions/2026-05-30-daemon-prs-orphan-verdict.md`: this repo
ships the contract and SDK; operator runtime and persona apps live in
`mutav-app`.

Loop in `mutav-app` maintainers when:

- A SDK CVE requires consumer upgrades or UI copy changes.
- A contract behavior change affects Convex Actions or persona apps.
- A containment action requires `apps/admin` hardware-wallet signing.
- An incident affects the agency payment-address display surface.

Open a private or public tracking issue on `mutav-app` after disclosure timing is
settled. Convex Action implementations `mutav-app#141` through `#146` should
reference this runbook when documenting inherited vs. mitigated risks.

## Disclosure Timeline

Default coordinated disclosure target: 90 days from confirmed triage, or sooner
after a fix is released and downstream consumers are safe.

Shorten the timeline when:

- The issue is actively exploited.
- Public chain data already reveals the exploit class.
- A fixed release or mitigation is available and users need immediate action.

Extend the timeline only when a contract migration, audit review, or downstream
consumer rollout requires more time and the reporter agrees.

Public advisories should include severity, affected versions or deployments,
impact, mitigation, patched version or contract deployment, reporter credit
preference, and follow-up issues.

## Roles

| Role | Responsibility |
|---|---|
| Intake maintainer | Owns the GitHub private advisory, reporter communication, and initial severity estimate. |
| Contract maintainer | Reproduces and patches `Fund` contract issues; prepares migration or deployment notes. |
| SDK maintainer | Reproduces and patches SDK issues; coordinates npm release and deprecation/yank decisions. |
| Admin signer | Performs `set_paused`, `set_operator`, `propose_admin`, or other admin transactions from `mutav-app/apps/admin/`. |
| `mutav-app` coordinator | Handles Convex Action, persona-app, and consumer rollout work when the incident crosses repositories. |

## Post-Incident

After disclosure:

1. Publish the GitHub Security Advisory or release note.
2. Write a post-mortem covering root cause, detection, timeline, blast radius,
   containment decisions, and user impact.
3. Walk the issue against the threat model from `mutav-stellar#46`; if that
   model is not merged yet, record the missing threat as a follow-up.
4. File follow-up issues for mitigation gaps, at minimum linking relevant
   contract, SDK, observability, key-custody, or readiness work such as #28,
   #30, #31, #32, #33, #34, #40, #41, #43, #44, #46, #47, and #49.
5. Update this runbook if responders had to improvise a step.
