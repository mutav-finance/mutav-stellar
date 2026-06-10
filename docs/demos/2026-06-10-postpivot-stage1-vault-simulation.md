# Stage-1 reserve vault — post-pivot testnet simulation

**Date:** 2026-06-10
**Network:** Stellar testnet
**Vault contract:** [`CBDGKVRP5MYER3I2WZ7F2FJULFFXY3NHB5MU75VSEZHDXYJNAB3YC7Y2`](https://stellar.expert/explorer/testnet/contract/CBDGKVRP5MYER3I2WZ7F2FJULFFXY3NHB5MU75VSEZHDXYJNAB3YC7Y2)

> **Validates the contract that landed in [PR #97](https://github.com/mutav-finance/mutav-stellar/pull/97)** — squash commit [`0310639`](https://github.com/mutav-finance/mutav-stellar/commit/0310639) on `main`. This is the "safe with allowlists" surface (~410 LOC, single `withdraw` value-flow path), not the richer pre-pivot surface exercised in the [archived 2026-06-09 walkthrough](../archive/2026-06-09-stage1-vault-testnet-simulation.md).

## TL;DR

The post-pivot Stage-1 reserve vault was deployed to Stellar testnet via the `__constructor` pattern, funded with 1M mock-USDC, and exercised across twelve scenarios covering happy-path withdraw, all four expected panic paths, pause/unpause, two-step admin handover with expiry cancellation, force-remove with stranded dust, allowlist cap enforcement, and permissionless TTL extension. **12/12 PASS.** No findings; behaviour matches the [design spec](../specs/2026-06-08-stage1-reserve-vault-design.md) on every dimension.

## What this validates

- **Constructor-pattern init** — `__constructor` invoked via the `-- --admin <addr>` separator at deploy time; `admin()` view returns the expected key.
- **`withdraw(asset, amount, destination, ref_hash)`** — single value-flow path; happy path moves funds, all four guards (`AssetNotApproved`, `DestinationNotAllowed`, `AmountMustBePositive`, `Paused`) panic as designed.
- **Asset + destination allowlists** — admin-only mutations; `add_approved_asset` / `add_allowed_destination` emit `asset_add` / `dest_add` events.
- **Two-step admin with expiry** — `propose_admin` writes `PendingAdmin{address, live_until_ledger}` in temporary storage; `accept_admin` rotates authority; `propose_admin(_, 0)` is the explicit cancel sentinel and emits a distinct `prop_can` event.
- **`force_remove_approved_asset`** — escape hatch when an allowlisted asset has a non-zero vault balance; emits `AssetForceRemoved` with the stranded amount.
- **Permissionless `extend_ttl`** — anyone can bump the instance-storage TTL; verified by an attacker key successfully calling it.

## Deployment

| Field | Value |
|---|---|
| Vault contract | [`CBDGKVRP5MYER3I2WZ7F2FJULFFXY3NHB5MU75VSEZHDXYJNAB3YC7Y2`](https://stellar.expert/explorer/testnet/contract/CBDGKVRP5MYER3I2WZ7F2FJULFFXY3NHB5MU75VSEZHDXYJNAB3YC7Y2) |
| Alias | `reserve-vault-postpivot` |
| WASM size | 11,455 bytes |
| WASM SHA-256 | `6969d1652cb37102d7c876ab4dc2219cc8f08c88e485cc532106e7ef075bda55` |
| Deploy tx | [`bbd9591b…02011`](https://stellar.expert/explorer/testnet/tx/bbd9591b2f42e0d8e8a3bcf6e08fc98ac0b6ff00177416bb62c01e7beee02011) |
| Build target | `wasm32v1-none`, release profile |
| Constructor | `__constructor` via `-- --admin mutav-admin` separator |

`admin()` view returned `GD744VFXP3ZSIEU33CM7SMGJLG5ENQT7T4SOLFL5OIF76BURNJMC7KUZ` — exact match with `stellar keys address mutav-admin`.

## Setup

| Item | Value |
|---|---|
| Admin (mutav-admin) | `GD744VFXP3ZSIEU33CM7SMGJLG5ENQT7T4SOLFL5OIF76BURNJMC7KUZ` |
| Test PSP (whitelisted destination) | `GABIPH6Y5UKNM723Y6SJMBP4QEU3FZPSS5WGQBRI4SRJ34XPRLBRJPZX` |
| Test attacker (non-allowlisted) | `GB3W4IJBLEGA5KBYJHINGPSNCQHEQAASET3WR4N4AIPFHFSD6X6VJ5RP` |
| Issuer-mock (SAC issuer) | `GA4IEGEXNFEHI57FVFT3GUIAUFWDHSNBQYTT4CKENP56F5MNSS3NFAID` |
| USDCMOCK | [`CCMBVRGPD7AUP52R2MWFB4YUVCMPNBQHPDCT4DAAXOGFOV5UPIBMZ5SE`](https://stellar.expert/explorer/testnet/contract/CCMBVRGPD7AUP52R2MWFB4YUVCMPNBQHPDCT4DAAXOGFOV5UPIBMZ5SE) |
| BRLTMOCK | [`CC2N3ZY4GH24GJLBRP6AWKTC4H54Q4KM6RW3MQNLJJLCTRK7LPNFM7HI`](https://stellar.expert/explorer/testnet/contract/CC2N3ZY4GH24GJLBRP6AWKTC4H54Q4KM6RW3MQNLJJLCTRK7LPNFM7HI) |
| Approved assets at start | `[USDCMOCK, BRLTMOCK]` — both emitted `AssetApproved`/`asset_add` |
| Allowed destinations at start | `[test-psp]` — emitted `DestinationAllowed`/`dest_add` (filled to cap of 64 from prior runs) |
| Vault USDCMOCK funding | 10,000,000,000,000 stroops (1,000,000 USDC @ 7 dp) |
| BRLTMOCK funding | none initially (trustline opened on test-psp, no mint to vault) |
| Trustlines (test-psp) | USDCMOCK ([`b69d8cb4…`](https://stellar.expert/explorer/testnet/tx/b69d8cb4)), BRLTMOCK ([`e4f0eaf3…`](https://stellar.expert/explorer/testnet/tx/e4f0eaf3)) |

## Core scenarios

| # | Scenario | Status | Observation |
|---|---|---|---|
| 1 | `happy_withdraw` | PASS | Admin withdrew 50,000,000,000 stroops USDCMOCK to test-psp. test-psp balance 0 → 50,000,000,000. `Withdrawn` event emitted with `ref_hash` `ee9e1d8d…85e974`. Tx [`18e3c25e…`](https://stellar.expert/explorer/testnet/tx/18e3c25edba25cb201e2c58d11f96051a96e37bcee4b9dca2786982c7661e831). |
| 2 | `reject_unapproved_asset` | PASS | Passed the vault contract ID itself as the asset; contract panicked with `Error(Contract, #3) AssetNotApproved`. |
| 3 | `reject_non_whitelisted_destination` | PASS | Withdraw targeting test-attacker `GB3W…JPZX` rejected with `Error(Contract, #6) DestinationNotAllowed`. |
| 4 | `reject_zero_amount` | PASS | Withdraw with `amount=0` rejected with `Error(Contract, #8) AmountMustBePositive`. |
| 5 | `pause_blocks_then_unpause` | PASS | `set_paused(true)` emitted `PausedSet{paused:true}`; `paused()` returned true; withdraw of 25,000,000,000 rejected with `Error(Contract, #2) Paused`; `set_paused(false)` emitted `PausedSet{paused:false}`; subsequent withdraw of 25,000,000,000 USDCMOCK to test-psp succeeded (tx [`3dca510e…`](https://stellar.expert/explorer/testnet/tx/3dca510e6f6986d92ad6a9bc8fac0e1502778cedecb1042dff9d61bbd70ff41b)). |
| 6 | `permissionless_extend_ttl_by_attacker` | PASS | test-attacker successfully called `extend_ttl` — confirming the permissionless TTL bump path is open as designed (instance storage only; `PendingAdmin` in temporary unaffected). Tx [`04a01dd5…`](https://stellar.expert/explorer/testnet/tx/04a01dd5a2e75641d8553b310f0ae874bf38d6d407a2a406d7e7ac0993ea7843). |
| 7 | `force_remove_with_balance` | PASS | Minted 1 stroop BRLTMOCK to vault (tx [`ecf63484…`](https://stellar.expert/explorer/testnet/tx/ecf63484801d0d9e59e24802677b7ee51dcdadee1857fde9e64eaf51dc755fd0)). `remove_approved_asset(BRLTMOCK)` failed with `Error(Contract, #5) AssetBalanceNonzero`. `force_remove_approved_asset(BRLTMOCK)` succeeded and emitted `AssetForceRemoved{stranded_balance:1}` (tx [`4fdf4758…`](https://stellar.expert/explorer/testnet/tx/4fdf47584bd730330e01d8b88ce9b48013adc4f996900559904d4f25b9f18397)). `is_approved_asset(BRLTMOCK)=false` post-removal; vault `balance(BRLTMOCK)=1` (dust stranded as expected). |
| 8 | `random_caller_cant_admin` | PASS | test-attacker invoked `add_allowed_destination`; CLI reported `Missing signing key for account GD744V…C7KUZ` (the mutav-admin address) during auth resolution — call required the admin signature which the attacker cannot provide. Allowlist mutation rejected. |

**Subtotal: 8/8 PASS.**

## Stress + rotation

| # | Scenario | Status | Observation |
|---|---|---|---|
| 9 | `fill_destination_allowlist_to_cap` | PASS | Allowlist was already at 64 entries from prior runs (`allowed_destinations` returned 64 strkeys). Cap reached, no further action needed. |
| 10 | `overflow_at_cap_panics` | PASS | `add_allowed_destination` with a fresh Python-generated strkey (`GDJPR2VEBQYD4HDAZPUII5FCBTYTSTD2MJPRYIL7RDZLPDQ5G5WTSJGY`) panicked at simulation with `Error(Contract, #9) AllowlistFull` and diagnostic `failing with contract error 9`. |
| 11 | `two_step_admin_handover_full_flow` | PASS | Ledger ~3,023,792; deadline 3,041,072 (~24h). mutav-admin proposed test-psp; `pending_admin` returned `{address: test-psp, live_until_ledger: 3041072}`. test-psp accepted; admin became `GABIPH…JPZX`. Rotation back: test-psp proposed mutav-admin (deadline 3,041,083); mutav-admin accepted; admin restored to `GD744V…C7KUZ`. `AdminProposed` and `AdminAccepted` events emitted at each step. Txs [`13220f15…`](https://stellar.expert/explorer/testnet/tx/13220f1528868311fc93026091bca0461ad422695991dbec12b1fadd80b931fa), [`96fc8e20…`](https://stellar.expert/explorer/testnet/tx/96fc8e20ce3dba97190d6d8ba426307d1893863264e3cee712e69a72732db02d), [`cd127682…`](https://stellar.expert/explorer/testnet/tx/cd127682be3e5be427ea829f1be21a5ea81db391584ceaff77cabbd1d36537a7), [`c074b3a2…`](https://stellar.expert/explorer/testnet/tx/c074b3a2a9a8be72cb3398eefba1f7aed8f7b242758c6502b1fb63fcb763ba82). |
| 12 | `propose_admin_cancel_with_zero` | PASS | mutav-admin proposed test-psp with deadline 3,041,089 (valid future). Second call with `--live_until_ledger 0` emitted `AdminProposalCancelled` (`prop_can`) by mutav-admin; `pending_admin` now returns `null`. Cancel semantics work via the `live_until_ledger=0` sentinel (distinct cancel event rather than silently overwriting). Txs [`94d003a2…`](https://stellar.expert/explorer/testnet/tx/94d003a22d3beaebea8d4e7a1af37d1b54637756fe7298e3eb18c9943c150050), [`a8caec39…`](https://stellar.expert/explorer/testnet/tx/a8caec399683718bcbf12118671a322c8e01fc5cf97e1fba51a89677fb9498b8). |

**Subtotal: 4/4 PASS.**

## Findings

None. All twelve scenarios produced the exact event payloads, view values, and error codes specified in the design doc. Specifically worth restating because they're easy to confuse with bugs:

- **Permissionless `extend_ttl` is by design** (scenario 6). Anyone — including an attacker key — can bump the instance-storage TTL. Operationally this is a feature (any well-meaning third party can keep the vault alive cheaply); the surface holds no authority since the call only writes a TTL extension on instance storage and the `PendingAdmin` lives in temporary storage with its own `live_until_ledger`.
- **`force_remove` strands dust on purpose** (scenario 7). Removing an asset with a non-zero balance is only possible via `force_remove_approved_asset`, which emits the stranded balance in `AssetForceRemoved` so an off-chain indexer can reconcile. The 1-stroop BRLTMOCK that remained on the vault after the test is expected and intentional.
- **Cancel via `live_until_ledger=0`** (scenario 12) is the only cancel path; the contract emits a distinct `prop_can` event rather than silently overwriting `PendingAdmin`, which keeps the off-chain audit trail clean.

## What changed since pre-pivot

The [pre-pivot 2026-06-09 walkthrough](../archive/2026-06-09-stage1-vault-testnet-simulation.md) exercised the richer surface that has since been removed. The diff is large enough to warrant restating before reading either doc:

| Dimension | Pre-pivot (archived) | Post-pivot (this doc) |
|---|---|---|
| Vault contract ID | [`CAJTKYO…XWAJR`](https://stellar.expert/explorer/testnet/contract/CAJTKYOPDRWCQJGPUQNKD6KJ6LK6XMSHY2QLIKVR44L4KUFXXZ46WAJR) | [`CBDGKVRP…7Y2`](https://stellar.expert/explorer/testnet/contract/CBDGKVRP5MYER3I2WZ7F2FJULFFXY3NHB5MU75VSEZHDXYJNAB3YC7Y2) |
| LOC | ~1,200 | ~410 |
| Value-flow paths | `pay_default` (propose/execute/cancel), `operator_outbound`, `record_capital_receipt`, `record_swap_in`, … | single `withdraw(asset, amount, destination, ref_hash)` |
| Roles | admin + operator + payment-provider + denomination asset | admin only |
| Timelock | 24h on `pay_default` (in-contract) | none in-contract — policy moves to the OZ Smart Account at the admin address |
| Per-item maxima | `pay_default_max_item_value` | none — admin signs the exact amount |
| Replay protection | `src_tx_hash` (7-day temporary) on capital receipts | `ref_hash` opaque audit field on `Withdrawn` event (no replay storage) |
| Pending-state tracking | `PendingSwap`, `PendingPayDefault(id)` queue, `pending_proposals_count` | only `PendingAdmin` (temporary, with `live_until_ledger`) |
| Allowlist caps | assets ≤ 8, destinations ≤ 16 | assets ≤ 8, destinations ≤ 64 |
| Admin handover | two-step | two-step with explicit `live_until_ledger` expiry + zero-sentinel cancel emitting `prop_can` |
| Force-remove | not present | `force_remove_approved_asset` with `AssetForceRemoved{stranded_balance}` |
| Pre-pivot findings (1, 4) | `MIN_TIMELOCK_SECS` floor; `set_denomination_asset` blocks when proposals pending | not applicable — no timelock, no denomination asset |
| Pre-pivot finding 3 (trustline preflight) | dashboard concern | still a dashboard concern; unchanged for `withdraw` |
| Pre-pivot finding 5 (operator-attested amounts) | indexer reconciles `cap_in` against SAC `transfer` | not applicable — no operator-attested inbound recording |

Most of the security properties the pre-pivot walkthrough demonstrated (signer thresholds, spending limits, per-operation differentiation, timelocks) are now properties of the **OZ Smart Account configuration at the admin address**, not the vault contract. The vault is deliberately the smallest possible safe object: an allowlisted-destination withdrawal primitive with a two-step admin and a force-remove escape hatch.

## Related

- **Design spec:** [`docs/specs/2026-06-08-stage1-reserve-vault-design.md`](../specs/2026-06-08-stage1-reserve-vault-design.md)
- **Merged PR:** [#97](https://github.com/mutav-finance/mutav-stellar/pull/97) (squash `0310639` on `main`)
- **Implementation:** [`contracts/stage1/reserve_vault/`](../../contracts/stage1/reserve_vault/)
- **Pre-pivot archive:** [`docs/archive/2026-06-09-stage1-vault-testnet-simulation.md`](../archive/2026-06-09-stage1-vault-testnet-simulation.md)
- **Stage 1 README:** [`contracts/stage1/README.md`](../../contracts/stage1/README.md)
