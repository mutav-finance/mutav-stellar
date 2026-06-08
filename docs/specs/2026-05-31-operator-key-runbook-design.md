# Operator-key runbook — design

**Date:** 2026-05-31
**Branch:** `docs/operator-key-runbook`
**Status:** Approved (brainstorm), pending implementation plan.

## Context

`mutav-stellar` ships the Soroban `Fund` contract + read-oriented TypeScript SDK. Per [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57) and the [orphan-verdict ADR](../architecture/decisions/2026-05-30-daemon-prs-orphan-verdict.md), the six operational responsibilities (on-ramp, off-ramp, yield-sync, mgmt-fee, heartbeat, ttl-watchdog) move to `mutav-app` as KMS-backed Convex Actions. The operator key — which signs AUM-routing calls and TTL renewals on the contract — moves with them. This repo's deployment surface stops at the contract + SDK.

[`#41`](https://github.com/mutav-finance/mutav-stellar/issues/41) is the open ask for a key-custody runbook. The full issue spans admin (cold) storage, operator (hot) custody, rotation, incident, and mainnet bootstrap ceremony. **This spec covers the operator-key surface only** — the foundational document the 6 Convex Action implementations refer to when they get built. Admin cold storage (HW wallet inside `mutav-app/apps/admin/`) and the mainnet bootstrap ceremony are deferred to their own specs.

For the broader vulnerability-response loop that decides severity, containment,
patch staging, disclosure timing, and cross-repo coordination, see
[`docs/security/incident-response.md`](../security/incident-response.md).

The [ADR's Q4](../architecture/decisions/2026-05-30-daemon-prs-orphan-verdict.md) locked a hybrid operator-key strategy: shared key for low-risk renewal crons (heartbeat, ttl-watchdog), per-Action scoped keys for AUM-touching Actions (on-ramp, off-ramp, yield-sync, mgmt-fee). This spec captures the principle and defers the concrete per-Action mechanism to each Action's implementation spec — that question intersects each Action's specific authority surface and is better answered when those Actions are designed.

## Goal

Establish the foundational decisions and constraints the operator-key custody needs, so that:

- The 6 Convex Action implementations on `mutav-app` (mutav-app#141–#146) can reference one place for their signing model.
- Future audit reviewers see a documented separation between key material (KMS), signing authority (Convex Action via OIDC), and on-chain effect (Stellar operator authority on the `Fund` contract).
- The team can context-switch back to contract/SDK work without re-deriving these decisions every time an Action gets picked up.

## Non-goals

- **Concrete KMS provider selection.** AWS KMS / GCP KMS / HashiCorp Vault all satisfy the criteria listed in § 2. Provider gets picked when the first AUM-touching Action is built. The runbook is provider-agnostic so the Action layer codes against an interface, not a vendor.
- **Per-Action key-scoping mechanism.** The principle is "blast-radius isolation per AUM-touching Action." Whether that's distinct Stellar keys (would require contract changes — see § 1) or one Stellar key with per-Action IAM scoping is a decision each Action's design will make based on its specific authority surface.
- **Admin (cold) key custody.** Lives on `mutav-app/apps/admin/` as a HW-wallet flow; tracked separately.
- **Mainnet bootstrap key ceremony.** Pre-mainnet protocol-level concern; gets its own spec.
- **Observability / paging integration.** Tracked at [`#44`](https://github.com/mutav-finance/mutav-stellar/issues/44); referenced by § 5 but not designed here.
- **Per-Action specific signing call shapes.** Each Action's implementation spec defines its own signing call against the abstraction in § 2.

## Section 1 — Trust model & boundary statement

### Where each authority lives

| Authority | Material lives in | Identity owned by | On-chain effect |
|---|---|---|---|
| **Operator** | Cloud KMS (provider TBD; criteria in § 2) | `mutav-app` (Convex Action requests signature via OIDC bridge in § 3) | `receive_payment`, `process_redemptions`, `add_yield`, `charge_mgmt_fee`, `extend_ttl`, `extend_balance_ttl`, `fulfill_redemption`, `record_offchain_payout` — whatever the current contract surface authorizes to `operator` |
| **Admin** | Hardware wallet (`mutav-app/apps/admin/` flow — deferred spec) | `mutav-app/apps/admin/` (Auth0 `mutavStaff` connection, MFA-gated; HW wallet for signing) | `set_operator`, `cover_default`, partner whitelist, `pause` / `unpause`, admin handover |
| **Investor** | Browser wallet (per chain) | `mutav-app/apps/fund/` | Deposit, request/cancel redemption, SEP-41 token ops |

### Boundary rule (recap from CLAUDE.md)

> Operator/admin keys do not live in this repo's deployment surface. Operator authority lives in a KMS-backed Convex Action on `mutav-app`; admin authority on a hardware wallet inside `mutav-app/apps/admin/`. This repo's deployment is the on-chain contract and the published SDK.

The runbook describes what `mutav-app`'s Action layer must respect, not how `mutav-app` implements its UI. The on-chain side of every procedure here happens via this repo's contract; the off-chain side happens in `mutav-app`.

### One Stellar operator key — for now

The contract currently exposes a single `operator` address via `set_operator(addr)`. Any "per-Action scoped key" interpretation that requires distinct Stellar addresses needs a contract change (e.g. a per-function operator map or an authorized-operator whitelist). Such changes are audit-gated. **This spec does not commit to such a change.**

The implication for Convex Action design: the AUM-touching Actions (on-ramp, off-ramp, yield-sync, mgmt-fee) share one underlying Stellar key today. Per-Action blast-radius isolation, when each Action's spec adopts it, happens at the **KMS access layer** (per-Action IAM scoping on the same key) — not at the Stellar key-material layer — until and unless a contract change makes per-function operators feasible.

### What cannot happen

- The operator Ed25519 private key never leaves KMS.
- No Convex Action ever holds the raw private key in memory.
- The SDK in `src/` does not sign anything. It only composes XDR (per the [orphan-verdict ADR](../architecture/decisions/2026-05-30-daemon-prs-orphan-verdict.md) PR B).
- This repo's CI / dev / production deployments do not need any operator key material to function.

## Section 2 — KMS abstraction (provider-agnostic)

### Interface

The Convex Action layer codes against an interface, not a vendor:

```typescript
interface KmsSigner {
  // Returns an Ed25519 signature over the payload. Implementation calls
  // the KMS provider's signing API; key material never enters this process.
  signEd25519(payload: Uint8Array): Promise<Ed25519Signature>;

  // Returns the Ed25519 public key (raw 32 bytes). Used by callers to
  // derive the Stellar address (StrKey encoding) and verify the signer
  // identity at runtime.
  publicKey(): Promise<Ed25519PublicKey>;

  // Opaque handle identifying which key in KMS this signer maps to.
  // Provider-specific (AWS KMS key ARN, GCP key resource path, Vault key name).
  keyId(): KeyHandle;
}
```

Each AUM-touching Action receives a `KmsSigner` constructed at boot from environment configuration. The shared key for renewal crons (heartbeat, ttl-watchdog) is a `KmsSigner` instance configured against the shared key handle.

### Provider selection criteria

When the provider is picked (downstream spec), it must satisfy:

- **Native Ed25519 signing.** Provider exposes Ed25519 directly (no software hashing inside the KMS abstraction layer).
- **OIDC trust support.** The provider can issue short-lived credentials in response to a presented OIDC token from a trusted issuer (see § 3).
- **Per-key IAM.** Access policy can be scoped per key handle — at minimum so that the renewal-cron shared key is distinct from the AUM-touching key(s), and so that production keys are unreachable from dev/preview environments.
- **Mature audit trail.** Provider records every signing call with caller identity, timestamp, key handle. Records are queryable and exportable for audit.

AWS KMS, GCP KMS, and HashiCorp Vault (with Transit secrets engine) all meet these criteria. HSMs (CloudHSM, equivalent) also qualify and may be revisited if regulatory diligence requires.

### What is not in the abstraction

- **Key rotation flow.** Rotation is procedure (§ 4), not interface. The `KmsSigner` interface holds a single key handle for its lifetime; rotation rebuilds the signer with a new handle.
- **Credential exchange.** The OIDC bridge in § 3 lives outside the abstraction — `KmsSigner` implementations are constructed with whatever credentials the runtime hands them.

## Section 3 — OIDC bridge pattern (Convex Action → KMS)

### The trust chain

```
[Convex Action]
   │ Convex-internal bearer token (per-Action identity)
   ▼
[Vercel route — bridge]
   │ Vercel OIDC token (per Vercel project / per environment claim)
   ▼
[Cloud KMS provider]
   │ Short-lived credential scoped to the requested key handle
   ▼
[Sign payload] → return signature to bridge → return to Action → submit XDR to Stellar
```

The bridge lives on Vercel because:

- Vercel issues OIDC tokens natively per-project per-environment, with claims (`aud`, `sub`) that cloud KMS providers can use as trust anchors.
- The bridge is HTTP-only, no state. It receives a signing request, presents its Vercel OIDC token to KMS, and returns the signature. No long-lived secrets, no key material in the bridge process.

Where the bridge route lives in the `mutav-app` monorepo is its call — likely a Vercel route inside `apps/admin` initially (admin owns the operational surface), with extraction to its own `apps/auth` once a second consumer needs it.

### Two trust models — by environment

**Production**:
- Convex Action → bridge with Convex-internal bearer token (verified by bridge against a Convex env-set shared secret OR via Convex's JWT issuance if available at implementation time).
- Bridge → KMS with Vercel OIDC token. KMS's per-key IAM policy trusts the Vercel OIDC issuer + `sub` claim matching the bridge's identity.
- No long-lived KMS credential is ever held by any Mutav-operated process.

**Dev / preview**:
- Convex Action calls KMS directly using long-lived KMS API credentials stored in Convex env.
- Credentials are scoped to **dev-only KMS keys**. Production keys are unreachable.
- The OIDC bridge is bypassed entirely in dev/preview — intentional ergonomic concession.

The dev/prod gap is documented as deliberate, not vestigial. CI in `mutav-app` exercises both paths.

### What the bridge route surface looks like

Abstract description; concrete shape is a downstream Action implementation:

- One signing endpoint per Action namespace (e.g. `POST /api/operator/sign/on-ramp`), or one endpoint with the Action identity in the request body and per-endpoint IAM resolved at the OIDC layer. The choice depends on whether per-Action IAM granularity is wanted at the bridge layer or the KMS layer (see § 1's "per-Action scoped keys" deferral).
- **Authenticated**: Convex-internal bearer token verified per request. No anonymous access.
- **Rate-limited**: per-Action and per-environment. Limits should sit well above expected legitimate signing rate; alert on threshold approach (links to [`#44`](https://github.com/mutav-finance/mutav-stellar/issues/44) observability spec).
- **Audit-logged**: every signing call recorded with (Action identity, OIDC subject, KMS key handle, payload hash, timestamp). Logs survive the bridge restart.

## Section 4 — Rotation procedure

Seven-step procedure. Provider-agnostic; specific KMS commands resolve when the provider is chosen.

1. **Generate new Ed25519 key in KMS.** New key handle, distinct from current. New Stellar address derived from the new public key.
2. **Fund new Stellar address.** Small XLM amount for fees (`mutav-app/apps/admin/` triggers a friendbot / treasury transfer; on mainnet, the funding comes from the existing treasury).
3. **Admin calls `set_operator(new_addr)` from `apps/admin/` HW-wallet flow.** This is a contract-side privileged call; admin signs from cold storage. Once this transaction confirms, the contract recognizes the new operator address.
4. **Convex Actions reload `KmsSigner`.** Configuration flip — every AUM-touching Action and the shared renewal-cron pair point to the new key handle. Rolling restart per Action; Convex's Action scheduler tolerates short windows where the Action is unavailable.
5. **Verify all six operational responsibilities are signing with the new key.** Observability check ([`#44`](https://github.com/mutav-finance/mutav-stellar/issues/44)) — each Action's next scheduled run posts a signature, the public key recovered from the signature matches the new key, no Action is still signing with the old key.
6. **Revoke / archive old key in KMS.** Disable the old key handle. Provider-specific (AWS KMS: schedule deletion or set state DISABLED; GCP: destroy version; Vault: revoke). Do not delete immediately — keep archived for the audit trail and for emergency rollback if step 5 surfaces a stuck Action.
7. **Append rotation to audit log.** Operational audit log (location TBD when `docs/ops/` is created with the runbook itself — see § "Spec location"). Entry records: rotation timestamp, old key handle, new key handle, admin signer identity, link to the `set_operator` Stellar tx.

### Rollback

If step 5 surfaces a stuck Action, admin can call `set_operator(old_addr)` to revert. The old KMS key handle stays archived (step 6 is deferred) until rollback is no longer plausible.

## Section 5 — Operator-key incident response

Single procedure. Operator-key compromise is the only incident this spec covers; admin-key compromise and contract-layer incidents are out of scope.

1. **Page on-call.** Routing per `mutav-app` paging configuration (downstream of [`#44`](https://github.com/mutav-finance/mutav-stellar/issues/44)).
2. **Admin calls `pause()` from `apps/admin/` HW-wallet flow.** Contract-layer pause halts every operator-authorized function. Admin authority is required; HW-wallet flow runs in `apps/admin/`. Latency goal: pause confirmed within minutes of detection.
3. **Execute rotation (§ 4 steps 1–6).** Skip step 7 until step 4 below completes.
4. **Admin calls `unpause()`.** Only after the new key is verified active on every operational responsibility (§ 4 step 5). Unpause is admin-authorized.
5. **Post-mortem.** Standard format: detection, response timeline, root cause, contributing factors, follow-ups. Lands in the operational audit log (§ 4 step 7's destination).

### What this does and does not protect against

- **Protects against**: KMS-credential compromise where attacker can request signatures but cannot move the key out of KMS. Pause halts further signing; rotation moves the contract to a new operator that the attacker cannot sign for.
- **Does not protect against**: admin-key compromise (different incident, different spec). Concurrent admin + operator compromise (out of scope — depends on admin cold-storage spec).

## Spec location & runbook destination

- **This spec** (planning artifact, captures decisions and rationale): `mutav-stellar/docs/specs/2026-05-31-operator-key-runbook-design.md`.
- **Eventual operational runbook** (the document operators read when rotating or responding): `mutav-stellar/docs/ops/operator-keys.md`. To be authored after this spec is approved. Distinct from the broader `docs/ops/key-custody.md` that [`#41`](https://github.com/mutav-finance/mutav-stellar/issues/41) originally requested — admin / ceremony / cross-cutting scope is deferred.
- **Implementation plan** (how the runbook gets written): follow-up under `mutav-stellar/docs/plans/`.

## Open follow-ups

| Item | Tracked at | Notes |
|---|---|---|
| Concrete KMS provider selection | This spec § 2 | Picked when first AUM-touching Action is implemented |
| Per-Action key-scoping mechanism | Per-Action spec (one per #141–#146) | Each Action's authority surface drives the answer |
| Admin (cold) key custody spec | `mutav-app/apps/admin/` HW-wallet flow | Out of scope here |
| Mainnet bootstrap key ceremony | Pre-mainnet protocol spec (not yet filed) | Out of scope here |
| Observability for signing rate / latency / failure | [`#44`](https://github.com/mutav-finance/mutav-stellar/issues/44) | Referenced by § 3 (rate limits) and § 5 (paging) |
| Operational audit log location | TBD when the runbook itself lands | § 4 step 7, § 5 step 5 both write to it |
| Convex-internal bearer token mechanism | Convex Action implementation specs | § 3's "verified by bridge against a shared secret OR Convex JWT" decision |
| Possible contract change: per-function operator | Defer; revisit when a per-Action implementation forces the question | Audit-gated; would unblock real per-Action key-material isolation |

## References

- [`mutav-stellar#41`](https://github.com/mutav-finance/mutav-stellar/issues/41) — original key custody + rotation runbook ask (broader than this spec)
- [`mutav-stellar#57`](https://github.com/mutav-finance/mutav-stellar/issues/57) — architecture consolidation that moved the operator runtime to `mutav-app`
- [`mutav-stellar#44`](https://github.com/mutav-finance/mutav-stellar/issues/44) — observability spec (referenced by § 3 and § 5)
- [`docs/architecture/decisions/2026-05-30-daemon-prs-orphan-verdict.md`](../architecture/decisions/2026-05-30-daemon-prs-orphan-verdict.md) — ADR locking Q4 hybrid key strategy
- [`docs/architecture/05-operational-layer.md`](../architecture/05-operational-layer.md) — the six operational responsibilities and their Convex primitives
- [`docs/architecture/02-actors-and-trust.md`](../architecture/02-actors-and-trust.md) — full trust model (operator / admin / investor / system)
- [`mutav-app#139`](https://github.com/mutav-finance/mutav-app/issues/139) — monorepo migration spec (referenced by § 3 for the apps/admin bridge location)
- Convex Action issues: mutav-app#141 (on-ramp), #142 (off-ramp), #143 (yield-sync), #144 (heartbeat), #145 (mgmt-fee), #146 (ttl-watchdog) — consume this spec when they get picked up
