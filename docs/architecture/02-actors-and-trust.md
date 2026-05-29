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
| **mutav-app (agency platform)** | Surfaces "pay USDC to wallet X" instructions to agency users via its UI | **Routing trust**: a compromised app DB or display layer could misroute agency payments without touching any operator/admin key. Mitigation: addresses (`classic_wallet`, operator pubkey) must be read from chain via the SDK at display time — never cached from app DB |
| **mutav-fund (web3 portal — investor + fund mgmt)** | Constructs transactions for investors AND for protocol-team admin (fund management) to sign client-side. Admin UI features gated by an on-chain `admin()` check, not client-side state. | Routing trust: a compromised app could craft a malicious tx for the connected wallet to sign. Mitigation: transaction details rendered for wallet UI review; signer sees recipient + amount before approving. Admin signs via hardware wallet — last line of defense. |
| **Mutav treasury account (off-chain Stellar account)** | Distinct from operator/admin. Holds USDC pre/post Etherfuse conversion. Keypair (`TREASURY_SECRET`) lives on `mutav-app`'s Convex backend; signs SEP-10 + SEP-24 anchor-flow XDRs only. | Trust scoped to anchor flows (no contract calls). The signer wrapper validates the public key matches `STELLAR_MUTAV_SOURCE_ACCOUNT` env at construction. **Boundary**: the treasury keypair must never be used to sign Fund-contract operations — that's operator authority and lives on `mutav-stellar`. Today this is trust-by-convention; no compile-time or contract-side restriction exists. |
| **Etherfuse (corporate counterparty)** | Converts USDC ↔ TESOURO; reports yield | Counterparty risk; mitigated by Etherfuse's regulatory posture (not documented here) |
| **Etherfuse internal operator** | Human (or system) with USDC↔TESOURO authority on Etherfuse's side | Not surfaced on-chain; trust delegated to Etherfuse's operational discipline |
| **PIX rail** | BRL payouts for management fee distribution | Banking infrastructure; trust comes from licensed counterparties |
| **Wallet vendor** (Freighter / Albedo / WalletConnect / stellar-wallets-kit) | Holds investor / agency keys; produces signed transactions | Trust delegated to the vendor's key-storage and tx-display logic; out of scope for this repo but a real attack surface |
| **Auth0 (mutav-app dep)** | Agency-user authentication | SOC 2 trust dependency for `mutav-app` only; does not gate any chain-side authority |
| **Convex (mutav-app dep)** | Real-time backend for `mutav-app` | Same — BaaS trust dependency for the agency platform |
| **RPC provider** | Soroban RPC + Horizon access | Currently single-vendor (testnet: stellar.org; mainnet: validationcloud.io) — see #44 for multi-vendor plan |
| **Stellar validator set** | Underlying consensus | Trust delegated to the Stellar network itself; assumed honest-majority |

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

## Known gaps in the trust model

Architectural gaps — distinct from the contract-implementation bugs surfaced by the audit, which live in [`03-contract.md`](./03-contract.md#known-gaps).

- **Host-level key custody** undocumented — issue #41.
- **Hot key sprawl**: all 6 daemons share one operator secret — surfaces in #41.
- **Routing trust for sibling apps unstated**: there's no doc saying "addresses must be read from chain at display time, never cached in app DB". Add as a sibling-repo contributing rule.
- **mutav-fund tx-display contract unstated**: the connected wallet's UI is the last line of defense against a malicious tx; the dApp should never silently submit, always show recipient + amount. Especially load-bearing for admin operations signed via hardware wallet.
- **Treasury scope-of-use undocumented at boundary**: the treasury keypair on `mutav-app` could in principle sign any Stellar XDR. Today it's used in SEP-10/SEP-24 flows only, but nothing in code or on-chain restricts it. Either restrict the signer to scope-tagged use-cases or document the trust-by-convention loudly.
- **Etherfuse internal operator** is invisible to this trust model — relies entirely on off-chain governance.
- **No on-chain audit log** beyond Soroban events — pending the indexer in #44.
- **No threat model document** ties these actors to attack scenarios — issue #46.

Implementation gaps (contract-side, separate from the trust model):
- Admin powers are larger than the cold/hot split suggests — issues #30, #32.
- Operator powers include several AUM-debiting calls without rate limits — issues #28, #31, #33.
