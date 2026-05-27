# 02 — Actors and trust

Three roles act *inside* the contract; four parties act *outside* it. The auth matrix below is the contract's primary security boundary.

## On-chain actors

### Admin (cold wallet)
- Parameter setters (`set_*_bps`, `set_fulfill_window`, `set_paused`)
- `set_classic_wallet`, `set_operator`, `set_approved_partner`
- `cover_default` (debits AUM after a sinistro)
- Admin handover: `propose_admin` → `accept_admin` (two-step)

Intended storage: multisig or hardware wallet. Used infrequently. Host-level custody is not yet documented (issue #41).

### Operator (hot wallet)
- `receive_payment` (partner inflows)
- `process_redemptions`, `fulfill_redemption` (weekly queue)
- `add_yield`, `add_tenant_fee` (recording off-chain growth)
- `charge_mgmt_fee` (monthly debit, gated by `MIN_FEE_INTERVAL = 30 days`)
- `record_offchain_payout` (debits AUM, expected to match off-chain wire)
- `sweep_usdc` (move idle USDC out of the contract)
- `extend_ttl` (instance storage TTL)

Intended storage: HSM/KMS-wrapped, lives on daemon hosts. Currently a single keypair shared by all 6 daemons — **hot-key sprawl** (issue #41, #44).

### Investor
- `deposit_investor` (mint MUTAV against USDC)
- `request_redemption`, `cancel_redemption`
- `fulfill_redemption` (self, after operator processes the queue)
- `reclaim_expired_redemption` (self, after `fulfill_window` expires)
- SEP-0041: `transfer`, `transfer_from`, `approve`, `burn`, `burn_from`

Anyone holding MUTAV is implicitly an investor. No KYC enforced on-chain (out of scope here).

## Off-chain actors

| Party | Role | Trust assumption |
|---|---|---|
| **Partner imobiliária** | Pays monthly guarantee fee in USDC; whitelisted by admin | Identity bound to Stellar G-address; trust comes from off-chain partnership + on-chain whitelist |
| **Etherfuse** | Converts USDC ↔ TESOURO; reports yield to operator | Counterparty risk; mitigated by Etherfuse's regulatory posture (not documented here) |
| **PIX rail** | BRL payouts for management fee and mgmt-fee distribution | Banking infrastructure; trust comes from licensed counterparties |
| **RPC provider** | Soroban RPC + Horizon access | Currently single-vendor (testnet: stellar.org; mainnet: validationcloud.io) — see #44 for multi-vendor plan |

## Auth matrix (selected high-leverage calls)

| Function | Caller | Notes |
|---|---|---|
| `set_classic_wallet` | admin | **Single-step**; one call diverts all future inflows. Issue #32. |
| `cover_default` | admin | **No cap, no whitelist, no timelock**. One call can zero AUM. Issue #30. |
| `sweep_usdc` | operator | **No reserve check** — can drain USDC reserved for ready redemptions. Issue #28. |
| `record_offchain_payout` | operator | **No destination whitelist, no rate limit**. Issue #33. |
| `add_yield`, `add_tenant_fee` | operator | **Per-call cap only, no per-period rolling cap**. Issue #31. |
| `receive_payment` | operator | Whitelisted partner check; replay-guard in PR #22 (TTL math also flagged) |
| `charge_mgmt_fee` | operator | 30-day interval check; first call after deploy charges a full month (issue #34) |
| `extend_ttl` (instance) | operator | Should probably be permissionless (issue #34) |

## Known gaps

- **Host-level key custody** undocumented — issue #41.
- **Hot key sprawl**: all 6 daemons share one operator secret — surfaces in #41 and the observability gap (#44).
- **Admin powers** are larger than the cold/hot split suggests because several admin functions are single-step and unbounded — issues #30, #32.
- **Operator powers** include several AUM-debiting calls without rate limits — issues #28, #31, #33.
- **No on-chain audit log** beyond Soroban events — pending the indexer in #44.
