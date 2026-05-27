# 03 — The `Fund` contract

`contracts/fund/src/lib.rs` (2,817 LOC). Implements the SEP-0041 token interface plus fund-specific logic for partner payments, investor deposits, yield, fees, redemption queue, and default coverage.

## Storage tiers

Soroban has three storage classes with different TTL semantics and cost profiles. The contract uses all three:

| Tier | What lives here | Why |
|---|---|---|
| **Instance** | `admin`, `operator`, `classic_wallet`, `protocol_addr`, `usdc_token`, fee/cap config, `aum`, `total_supply`, `paused`, `pending_admin`, `last_fee_timestamp`, `weekly_exit_used`, `weekly_epoch` | Shared by every call; one TTL bumps everything |
| **Persistent** | `Balance(investor)`, `Allowance(from,to)`, `PendingRedemption(investor)`, `RedemptionQueue` (Vec), `ApprovedPartner(addr)` | Per-user state; survives across calls; ~30-day TTL leases |
| **Temporary** | `ReadyRedemption(investor)` (with `deadline`), `SeenTxHash(hash)` (PR #22 replay guard, ~2.9-day TTL) | Short-lived; auto-cleaned after expiry |

Heartbeat daemon (#25) extends instance TTL; TTL-watchdog (#27) extends per-investor balance TTL.

## Public surface — 40+ functions, grouped

### Lifecycle
- `initialize(...)` — one-shot setup of admin, operator, asset, wallets, fee config.

### Inflows (operator)
- `receive_payment(imobiliaria, amount, tx_hash)` — partner fee split: `protocol_fee_bps` → protocol; remainder → classic_wallet (AUM↑). Replay-guarded by `tx_hash`.
- `deposit_investor(investor, amount)` — investor mints MUTAV at current NAV; USDC → classic_wallet (AUM↑).

### Redemption queue (investor + operator)
- `request_redemption(investor, mutav)` — lock MUTAV in queue.
- `cancel_redemption(investor)` — withdraw from queue before processing.
- `process_redemptions()` *(operator)* — weekly batch (≤ `MAX_QUEUE_BATCH = 40` examined). Snapshots NAV at process time; moves entries to `ReadyRedemption` with a `deadline`. Returns total USDC owed.
- `fulfill_redemption(investor)` — investor (or operator) finalizes the payout after USDC is deposited.
- `reclaim_expired_redemption(investor)` — investor recovers their MUTAV if `deadline` passes without fulfillment.

### Accounting (operator)
- `add_yield(amount)` — record Etherfuse yield (AUM↑, capped by `max_aum_increase_bps` per call).
- `add_tenant_fee(amount)` — same shape, different event.
- `charge_mgmt_fee()` — monthly: AUM × `mgmt_fee_bps` debited; `MIN_FEE_INTERVAL = 30 days` enforced.
- `record_offchain_payout(amount, destination)` — debits AUM; expected to match an off-chain wire.

### Admin
- `cover_default(amount, destination)` — debits AUM after a sinistro.
- `set_*` — paused, fee bps, exit cap, fulfill window, classic wallet, operator, approved partner.
- `propose_admin` / `accept_admin` — two-step handover.

### Maintenance (anyone or operator)
- `extend_ttl()` *(operator)* — instance TTL bump.
- `extend_balance_ttl(investor)` *(permissionless)* — per-balance TTL bump.
- `extend_redemption_ttl(investor)` *(permissionless)* — per-pending-redemption TTL bump.
- `sweep_usdc(amount)` *(operator)* — move idle USDC out (currently unsafe; see #28).

### Views
- `nav`, `aum`, `total_supply`, `paused`, `mgmt_fee_bps`, `protocol_fee_bps`, `exit_cap_bps`, `max_aum_increase_bps`, `redemption_fee_bps`, `operator`, `admin`, `is_approved_partner`, `pending_redemption`, `ready_redemption`, `ready_redemption_deadline`, `queue_len`, `weekly_exit_available`.

### SEP-0041 token
- `transfer`, `transfer_from`, `approve`, `burn`, `burn_from`, plus `name`, `symbol`, `decimals`, `balance`, `allowance`. **Note**: `burn` proportionally reduces AUM with no USDC returned to the burner — user footgun (issue #34).

## Key invariants

- **NAV**: `NAV = AUM / total_supply` (initial 1.0 when `total_supply == 0`). NAV is locked at *process time*, not request time.
- **Supply conservation**: total MUTAV minted = `total_supply`; `request_redemption` locks tokens (still in supply); `process_redemptions` burns them.
- **Weekly exit cap**: `weekly_exit_used` ≤ `aum × exit_cap_bps / 10_000` per epoch (`WEEK_SECONDS`). Resets every epoch.
- **Replay**: `receive_payment` rejects same `tx_hash` within the temporary-storage TTL (PR #22; TTL math wrong per #22 review — 2.9 days, not 7).
- **Pause**: blocks `deposit_investor` and `request_redemption`. Does NOT block `cancel_redemption` or `reclaim_expired_redemption` — investors can always exit a paused fund (by design).

## NAV math (locations)

- `calc_mint(amount_usdc)` → `amount_usdc × supply / aum` (or `amount_usdc` if `supply == 0`). lib.rs:328-338.
- `calc_redeem(mutav)` → `mutav × aum / supply`. lib.rs:340-342.
- `nav()` view → `aum × 10_000_000 / supply` (7-decimal display). lib.rs:1034-1040.

## Events (selected)

| Topic | Payload | Where emitted |
|---|---|---|
| `rcv_pay` | `(amount, protocol_cut, fund_portion)` | `receive_payment` |
| `deposit` | `(amount_usdc, mutav_minted)` | `deposit_investor` |
| `req_rdmpt` | `(mutav_amount)` | `request_redemption` |
| `cncl_rdmt` | `(pending,)` | `cancel_redemption` |
| `rdy_rdmpt` | `(usdc_owed, deadline)` | `process_redemptions` (one per investor moved to ready) |
| `fulfill` | `(usdc_paid)` | `fulfill_redemption` |
| `reclaim` | `(mutav_restored)` | `reclaim_expired_redemption` |
| `yield_in`, `fee_in` | `(amount,)` | `add_yield`, `add_tenant_fee` |
| `mgmt_fee` | `(fee,)` | `charge_mgmt_fee` |
| `offchain` | `(amount,)`; topic includes destination | `record_offchain_payout` |
| `sweep` | `(amount,)` | `sweep_usdc` |
| `default` | `(amount,)`; topic includes destination | `cover_default` |
| `set_*`, `prop_adm`, `acc_adm`, `set_paus`, `set_part`, `set_wall`, `set_op` | (varies) | admin setters |

Future indexer (#44) consumes these to power monitoring + audit log.

## Known gaps

- 2,817 LOC, **zero rustdoc comments** — issue #48.
- Test coverage gaps (auth-required tests, property invariants, sparse setters) — issue #39.
- Multiple admin/operator powers lack caps and timelocks — issues #28, #30, #31, #32, #33.
- No upgrade or migration path — issue #34.
- TTL math error in PR #22's replay guard — see PR #22 review.
