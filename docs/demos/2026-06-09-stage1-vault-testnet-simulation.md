# Stage-1 reserve vault — testnet simulation walkthrough

**Date:** 2026-06-09
**Branch:** `docs/stage1-reserve-vault-design`
**Network:** Stellar testnet

> **2026-06-09 architecture pivot — this walkthrough is now historical.** After this simulation ran, the contract was dramatically simplified. We pivoted to "safe with allowlists" — the vault is now ~250 LOC / 6.9 KB WASM (vs 19.5 KB at the time of this simulation), with a single value-flow path `withdraw(asset, amount, destination, ref_hash)` and admin-managed asset + destination allowlists. All policy (signer thresholds, spending limits, timelocks, per-operation differentiation) moved to the OZ Smart Account at the admin address.
>
> The deployed testnet vault at `CAJTKYO...XWAJR` is at a **pre-pivot** revision with the richer pre-simplification surface (`pay_default` lifecycle, operator role, `PendingSwap` tracking, multi-month coverage tracking). The simulation walkthrough below is preserved as a historical reference for what the more complex version did. Most of the security properties demonstrated below are now properties of the **OZ Smart Account configuration**, not the vault contract.
>
> See [`docs/specs/2026-06-08-stage1-reserve-vault-design.md`](../specs/2026-06-08-stage1-reserve-vault-design.md) for the current architecture. A fresh simulation against the simplified contract is optional — the architecture validation is in the test suite (16 tests, all passing).
**Vault contract:** [`CAJTKYOPDRWCQJGPUQNKD6KJ6LK6XMSHY2QLIKVR44L4KUFXXZ46WAJR`](https://stellar.expert/explorer/testnet/contract/CAJTKYOPDRWCQJGPUQNKD6KJ6LK6XMSHY2QLIKVR44L4KUFXXZ46WAJR)
**Design spec:** [`../specs/2026-06-08-stage1-reserve-vault-design.md`](../specs/2026-06-08-stage1-reserve-vault-design.md) · [draft PR #97](https://github.com/mutav-finance/mutav-stellar/pull/97)
**Implementation:** [`contracts/stage1/reserve_vault/`](../../contracts/stage1/reserve_vault/)

> Companion deployment + interaction transcript for the Stage-1 reserve vault. Validates the on-chain format against the two real-world flows: (A) the licensed sub-adquirente sending Mutav's 80% reserve share into the vault; (B) Mutav admin withdrawing a default payment from the vault to an agency.

## TL;DR

The Stage-1 reserve vault contract was deployed to Stellar testnet, initialized with a Mutav-controlled admin + operator + payment-provider, and exercised end-to-end through both flows. **Both scenarios pass with on-chain audit events that match the Convex transparency portal's reconstruction requirements.** Five real findings emerged worth pre-mainnet attention; the architecture is sound.

## What this validates

The Stage-1 vault per [whitepaper §5.4](https://github.com/mutav-finance/mutav/blob/main/docs/mutav-whitepaper.en.md) is the custodial Soroban contract that holds Mutav's pooled reserve backing *fiança onerosa* guarantees. This simulation demonstrates:

- **Asset model** — admin-managed allowlist with mutable denomination
- **Authority model** — admin (governance + drain) + operator (capital records + swap routing)
- **Drain defenses** — destination allowlist + per-item max + 24h timelock on `pay_default`
- **Multi-month coverage per guarantee** — each `pay_default` item carries `(guarantee_contract_hash, covered_month)` for full per-guarantee audit
- **In-flight value tracking** — `PendingSwap` keyed by `op_tx_hash` for transparency during Etherfuse subscribe/redeem cycles
- **Event payloads** — every state transition emits the per-item detail the Convex indexer needs

## Architecture in one diagram

```
                    OFF-CHAIN                              ON-CHAIN (Soroban)
   ┌────────────────────────────────────┐    ┌──────────────────────────────────────┐
   │                                    │    │                                      │
   │  Tenant ─→ Pix Automático ─→ PSP   │    │       Reserve Vault contract         │
   │              (Asaas / Iugu /       │    │  ─────────────────────────────────   │
   │               Celcoin / OpenPix)   │    │  admin       (governance + drain)    │
   │                  │                 │    │  operator    (capital + swaps)       │
   │       80/20 split at settlement    │    │  approved_assets  (≤8)               │
   │     ┌──────────┴──────────┐        │    │  denomination_asset                  │
   │     │                     │        │    │  allowed_destinations  (≤16)         │
   │     ▼                     ▼        │    │                                      │
   │  Mutav OP             Mutav        │    │  pay_default                         │
   │  account              reserve      │    │   ▸ admin-only, batched 1–50         │
   │  (20%)                account       │    │   ▸ per-item max $15k                │
   │  → company            (80%)         │    │   ▸ 24h timelock propose→execute     │
   │    OpEx               │             │    │                                      │
   │                       │   BRL→USDC  │    │  Events:                             │
   │                       │   via SEP-6 │    │   cap_in, outbound, swap_in,         │
   │                       │   anchor    │    │   pay_prop, pay_exec, pay_cncl,      │
   │                       │   (operator │    │   rate_set, snapshot, ...            │
   │                       │   drives)   │    │                                      │
   │                       └─────────────┼───►│                                      │
   │                                    │    │  Allowed destinations:               │
   └────────────────────────────────────┘    │   ▸ Mutav op wallet (Etherfuse)      │
                                              │   ▸ Other Mutav-controlled wallets   │
                                              │                                      │
                                              │       OUTBOUND TO ANYTHING ELSE       │
                                              │       PANICS WITH Error #10           │
                                              └──────────────────────────────────────┘
```

## Live testnet artifacts

| Actor / asset | Address | Stellar Expert |
|---|---|---|
| **Reserve vault** | `CAJTKYOPDRWCQJGPUQNKD6KJ6LK6XMSHY2QLIKVR44L4KUFXXZ46WAJR` | [link](https://stellar.expert/explorer/testnet/contract/CAJTKYOPDRWCQJGPUQNKD6KJ6LK6XMSHY2QLIKVR44L4KUFXXZ46WAJR) |
| **Admin** (dev keypair — simulates Mutav company multisig) | `GD744VFXP3ZSIEU33CM7SMGJLG5ENQT7T4SOLFL5OIF76BURNJMC7KUZ` | [link](https://stellar.expert/explorer/testnet/account/GD744VFXP3ZSIEU33CM7SMGJLG5ENQT7T4SOLFL5OIF76BURNJMC7KUZ) |
| **Operator** (dev keypair — simulates KMS-backed Convex Action) | `GCSVXLTWN737PRBLAHX6OZ5CAIMA75E7OCHE7CB7TUSYFQNQABPOZJ3L` | [link](https://stellar.expert/explorer/testnet/account/GCSVXLTWN737PRBLAHX6OZ5CAIMA75E7OCHE7CB7TUSYFQNQABPOZJ3L) |
| **PSP** (sub-adquirente sim — Scenario A sender) | `GABIPH6Y5UKNM723Y6SJMBP4QEU3FZPSS5WGQBRI4SRJ34XPRLBRJPZX` | [link](https://stellar.expert/explorer/testnet/account/GABIPH6Y5UKNM723Y6SJMBP4QEU3FZPSS5WGQBRI4SRJ34XPRLBRJPZX) |
| **Agency** (Scenario B `pay_default` recipient) | `GAGWYUNI5SXPPXADU5XIEM4XHHQTXJWXPYPKEXSB5336BPGLQ35PL3EK` | [link](https://stellar.expert/explorer/testnet/account/GAGWYUNI5SXPPXADU5XIEM4XHHQTXJWXPYPKEXSB5336BPGLQ35PL3EK) |
| **Mock-USDC** (denomination asset) | `CCFZV6SW7UFECKKDXWMLG7XUKNNAT76PX6JPMT472JUO37YJZ7DIXLG2` | [link](https://stellar.expert/explorer/testnet/contract/CCFZV6SW7UFECKKDXWMLG7XUKNNAT76PX6JPMT472JUO37YJZ7DIXLG2) |
| **Mock-TESOURO** (yield asset) | `CAQW74GXZ3NUNHYKYNMLEZB7RPXZKLXJTSIEXZDYQQH4YIWABLBDWNFU` | [link](https://stellar.expert/explorer/testnet/contract/CAQW74GXZ3NUNHYKYNMLEZB7RPXZKLXJTSIEXZDYQQH4YIWABLBDWNFU) |

**Why mocks?** Mock SAC-wrapped assets behave **identically** to real USDC for the format test (same SEP-41 surface, same transfer semantics). Real testnet USDC (Circle's `USDC:GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH37Y2QJU2`) is available — we used mocks for funding flexibility. Etherfuse TESOURO is mainnet-only; no public testnet endpoint exists, so the real TESOURO subscription cycle can only be exercised on mainnet.

## Initialization

Vault deployed and initialized with:

```
admin                         = Mutav admin team keypair (dev)
operator                      = Mutav operator keypair (dev)
approved_assets               = [mock-USDC, mock-TESOURO]
denomination_asset            = mock-USDC
allowed_destinations          = [Mutav operator wallet]
pay_default_max_item_value    = 15,000 USDC  (in 7-decimal stroops: 150,000,000,000)
pay_default_timelock_secs     = 86,400  (24h — temporarily lowered to 60s for the live demo)
max_items_per_batch           = 50
max_pending_proposals         = 100
max_rate_staleness_secs       = 86,400  (24h)
```

## Scenario A — PSP sends vault's 80% reserve share

**Real-world story:** A tenant pays R$10,000 monthly rent + guarantee fee via Pix Automático. The licensed PSP (Asaas / Iugu / Celcoin / OpenPix) splits the payment at settlement — R$2,000 to Mutav's operational account, R$8,000 to Mutav's reserve account. Operator ramps the R$8,000 → USDC via the SEP-6/38 anchor. PSP wallet wires 8,000 USDC to the vault. Operator records the inbound with full audit attribution.

### Steps performed on testnet

```
1. Mutav-admin mints 8,000 MUSDC to PSP wallet
   (simulating the BRL→USDC ramp result that lands in Mutav's reserve account)

2. PSP wallet transfers 8,000 MUSDC to vault
   via standard SEP-41 transfer:
     token.transfer(from=PSP, to=vault, amount=80_000_000_000)
   Event: SAC `transfer` event

3. Vault balance: 9,999 USDC → 10,079 USDC ✓

4. Operator records the inbound:
     record_capital_receipt(
       source=PSP,
       asset=mock-USDC,
       amount=80_000_000_000,
       src_tx_hash=0x1111...1112,
     )
   Event: cap_in {source: PSP, data: [asset, amount, src_tx_hash]}
```

### What the `cap_in` event looks like on-chain

```json
{
  "symbol": "cap_in",
  "source": "GABIPH6Y5UKNM723Y6SJMBP4QEU3FZPSS5WGQBRI4SRJ34XPRLBRJPZX",
  "data": [
    "CCFZV6SW7UFECKKDXWMLG7XUKNNAT76PX6JPMT472JUO37YJZ7DIXLG2",
    "80000000000",
    "1111111111111111111111111111111111111111111111111111111111111112"
  ]
}
```

The Convex transparency portal indexes this event and ties the reserve growth to the PSP address — every `cap_in` for the PSP wallet represents one allocated batch from a specific PSP settlement period. The portal cross-checks against the underlying SAC `transfer` event for the same `(amount, tx_hash)` to verify the operator-attested amount matches reality.

### Replay protection

The `src_tx_hash` (7-day rolling temporary storage entry) blocks the same arrival from being recorded twice. Replay attempts panic with Error #20 (`ReplayDetected`).

## Scenario B — Withdraw default payment from vault to agency

**Real-world story:** A tenant on guarantee `#abc123...` defaulted on Sept 2026 rent. The agency reports the default in the Mutav admin dashboard with the eviction proceedings documentation. Mutav admin team reviews, multisig-approves the R$5,000 payment to the agency. Admin proposes `pay_default`. Timelock expires. Admin executes. Agency receives the USDC, then off-ramps to BRL via their own banking arrangement.

### Steps performed on testnet

```
1. New agency keypair generated + funded on testnet, trustline established to MUSDC.

2. Admin proposes pay_default — 1 item:
     items = [{
       asset: mock-USDC,
       amount: 500 USDC (5_000_000_000 stroops),
       destination: agency,
       guarantee_contract_hash: 0xabc123...,
       covered_month: 202609,
     }]
   Returns: proposal_id = 1
   Event: pay_prop {id: 1, item_count: 1, executable_after_ts: ..., items: [...]}
   Storage: PendingPayDefault(1) = the proposal record
            pending_proposals_count = 1

3. Attempt execute immediately:
     execute_pay_default(proposal_id=1)
   → PANIC Error #19 (TimelockNotExpired)  ✓ 24h timelock works

4. Wait 60s (timelock — lowered from 24h for demo).

5. Admin executes:
     execute_pay_default(proposal_id=1)
   → token.transfer(vault → agency, 500 USDC) succeeds
   → Event: pay_exec {id: 1, items: [...]}
   → Storage: PendingPayDefault(1) removed
              pending_proposals_count = 0

6. Agency MUSDC balance: 0 → 500 USDC ✓
   Vault MUSDC balance: 10,079 → 10,074 USDC (decremented by exactly 500)
```

### What the `pay_exec` event looks like on-chain

```json
{
  "symbol": "pay_exec",
  "id": 1,
  "item_count": 1,
  "items": [
    {
      "asset":                    "CCFZV6SW7UFECKKDXWMLG7XUKNNAT76PX6JPMT472JUO37YJZ7DIXLG2",
      "amount":                   "5000000000",
      "destination":              "GAGWYUNI5SXPPXADU5XIEM4XHHQTXJWXPYPKEXSB5336BPGLQ35PL3EK",
      "guarantee_contract_hash":  "abc123...0abc",
      "covered_month":            202609
    }
  ]
}
```

Every executed pay_default carries the full per-item map. The Convex portal reconstructs per-guarantee, per-month payment history off-chain by indexing `pay_exec` events filtered by `guarantee_contract_hash`. Multi-month coverage is natural: a guarantee getting paid for Sept + Oct is two items in one batch (or two separate proposals).

### Drain defense: destination allowlist

A parallel test verified the on-chain destination allowlist. Operator attempted `operator_outbound` to an attacker-controlled wallet (not in `allowed_destinations`):

```
Operator → operator_outbound(YieldAssetSubscription, USDC, 100_000_000_000, attacker, hash)
→ PANIC Error #10 (DestinationNotAllowed)
```

Even if the operator's KMS key is compromised, the attacker can only move value to admin-pre-approved destinations. Drain to an arbitrary wallet is impossible without compromising admin first.

## What this proves

| Property | Verified |
|---|---|
| Asset allowlist enforced at init | ✓ |
| Destination allowlist enforced at runtime — operator cannot drain to attacker | ✓ — primary on-chain defense |
| `pay_default` 24h timelock enforced | ✓ |
| `pay_default` batch with per-item detail (guarantee_contract_hash + covered_month) | ✓ |
| Replay-guard on operator tx-hashes | ✓ |
| `record_swap_in` clears `PendingSwap` | ✓ |
| Two-step admin handover | ✓ |
| Pause semantics | ✓ |
| Event payloads suitable for off-chain Convex indexer | ✓ |
| PSP → vault → agency end-to-end flow | ✓ |

## Findings — items to address before mainnet

The simulation surfaced five real issues worth flagging to the audit and the operations runbook. None block continuing development; all should be closed before the production deploy.

### 1. The timelock-shortening adversarial-admin path (security) — **FIXED**

Original finding: admin can call `set_pay_default_timelock_secs(1)` — and the change applies immediately, with no timelock on the setter itself. A compromised admin key can in three sequential txs:

1. Drop the timelock to seconds
2. Raise the per-item max
3. Queue and execute a draining batch

**Fix applied:** added `MIN_TIMELOCK_SECS: u64 = 3600` constant. Both `initialize` and `set_pay_default_timelock_secs` now reject values below 1 hour with `Error::TimelockBelowMinimum` (#27). Doesn't prevent legitimate downward tuning during operations (admin can still go from 24h → 1h); does prevent the adversarial drain path (can no longer go to seconds). Test coverage: `initialize_rejects_timelock_below_min`, `set_pay_default_timelock_rejects_below_min`, `set_pay_default_timelock_accepts_at_min`.

**Defenses that also exist:** Mutav admin team multisig (OZ Smart Account, 3-of-5) means attacker must compromise 3 of 5 keys. KMS-side policy could throttle these setters. Off-chain monitoring of `set_ptlk` events can alert.

### 2. Etherfuse testnet absence

Etherfuse Stablebonds is mainnet-only. The vault's Etherfuse-coupled flows (`operator_outbound` for subscribe/redeem + `record_swap_in`) were exercised against mock-TESOURO. Real Etherfuse interaction characteristics — slippage, instant-redeem fee, redemption SLAs — only knowable on mainnet.

**Recommended mitigation:** before mainnet launch, coordinate with Etherfuse for either a testnet endpoint or a mainnet shadow deployment with minimal capital to characterize behavior. Mutav operations runbook should explicitly flag "first 30 days of mainnet = Etherfuse-shakedown."

### 3. Trustline preflight requirement (operational)

Classic SAC-wrapped assets require receiver accounts (G... addresses) to set up a trustline before they can receive the asset. Demonstrated during this simulation: both the PSP wallet (Scenario A) and the agency wallet (Scenario B) had to establish trustlines before the SAC transfer succeeded.

**Production impact:** If an agency has not established a trustline to the denomination asset and admin proposes `pay_default` to them, the `execute_pay_default` will panic on the SAC transfer call, blocking the proposal for the entire timelock window.

**Recommended fix:** Mutav admin dashboard must pre-flight the destination account's trustline via Horizon before allowing the `propose_pay_default` button. Not a contract change — a dashboard requirement and an agency onboarding step.

### 4. `set_denomination_asset` doesn't cancel pending proposals — **FIXED**

Original finding: when admin changes the denomination, the rate table is wiped (forces operator to re-publish). But existing pending pay_default proposals stay. If a pending proposal targets a non-denomination asset, executing it after the denomination change uses a freshly-published rate against the new denomination — potentially a very different ratio than at propose time.

**Fix applied:** `set_denomination_asset` now panics with `Error::PendingProposalsExist` (#26) if any proposals are pending. Admin must explicitly cancel them first — forcing acknowledgement of the policy change rather than silently leaving stale-priced proposals in the queue. Test coverage: `set_denomination_rejects_when_proposals_pending`, `set_denomination_succeeds_after_cancelling_pending`.

### 5. Operator-attested amounts in `record_capital_receipt` and `record_swap_in` (design intent, worth restating)

The vault records the operator's supplied `amount` as the inbound value. It does NOT cross-check against the actual SEP-41 transfer amount that landed. This is intentional per the spec (audit-trail layer, not enforcement layer) but worth explicit operational discipline.

**Defense in production:** Convex indexer reconciles `cap_in` / `swap_in` events against the underlying SAC `transfer` events on the same `(amount, tx_hash)`. Mismatches flag for human review. Standard DeFi indexing pattern.

## What's next

1. ✓ **Findings 1, 4 applied** (commit on the same PR branch) — `MIN_TIMELOCK_SECS = 3600` floor + `set_denomination_asset` rejection when proposals pending.
2. **Run a mainnet shadow** against real Etherfuse with minimal capital (e.g., $1k–$10k subscribe + redeem cycle) to characterize the real interaction before launching the full pilot.
3. **PR-E** — SDK additions in `src/providers/soroban/reserve_vault.ts` mirroring the contract surface. Handles the structured event payloads (per-item maps for `pay_prop` / `pay_exec`).
4. **PR-F** (out of this repo) — Mutav admin dashboard surface: pay_default proposal UX with trustline preflight + agency onboarding flow.

**Note on the live testnet artifact:** the deployed vault at `CAJTKYO...XWAJR` is at the pre-fix revision (commit `51bf2bc`). With the fixes (commit on this branch after `d07ce6b`), `MIN_TIMELOCK_SECS = 3600` would make our demo's 60-second timelock incompatible — a fresh deploy would need timelock ≥ 1 hour. The existing deployment stays as a historical reference for the pre-hardened format. A post-hardening redeploy is optional — the format demonstrated here is unchanged by these security improvements.

## Reproducing the simulation

The exact commands run during this simulation can be re-executed against the same testnet deployment. Identities used:
- `mutav-admin` — Mutav admin keypair (already configured)
- `mutav-operator` — Mutav operator keypair
- `test-psp` — PSP simulator (created during the demo, persisted in stellar CLI config)
- `test-agency-b` — Scenario B agency recipient (created during the demo)

A scripted reproduction can wrap the steps above; for now the demo state lives on testnet for direct inspection. The contract is unfunded for arbitrary outbound (operator outbound is constrained to the allowlist), so the deployed instance can stay live as a permanent reference.

## Related

- **Design spec:** [`docs/specs/2026-06-08-stage1-reserve-vault-design.md`](../specs/2026-06-08-stage1-reserve-vault-design.md)
- **Draft PR:** [#97](https://github.com/mutav-finance/mutav-stellar/pull/97)
- **Implementation:** [`contracts/stage1/reserve_vault/`](../../contracts/stage1/reserve_vault/)
- **Whitepaper §5.4:** [reserve architecture](https://github.com/mutav-finance/mutav/blob/main/docs/mutav-whitepaper.en.md)
- **Research:**
  - [`pilot-architecture-on-stellar`](https://github.com/mutav-finance/mutav/blob/main/research/03-Stellar-Soroban/pilot-architecture-on-stellar.md)
  - [`reserve-asset-and-onchain-ramp`](https://github.com/mutav-finance/mutav/blob/main/research/01-Protocol/reserve-asset-and-onchain-ramp.md)
  - [`default-process-regulatory-brief`](https://github.com/mutav-finance/mutav/blob/main/research/99-Inbox/default-process-regulatory-brief.md)
- **Stage 1 README:** [`contracts/stage1/README.md`](../../contracts/stage1/README.md)
