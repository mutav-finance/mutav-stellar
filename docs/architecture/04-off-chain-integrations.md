# 04 — Off-chain integrations

Three trust boundaries that the on-chain contract cannot enforce: Etherfuse (reserve provider), PIX (BRL rail), and partner Horizon accounts (USDC inflow). Plus the RPC vendor for the Convex Actions that run the operator runtime.

## Etherfuse — the reserve provider

Etherfuse converts USDC ↔ TESOURO and accrues yield on TESOURO holdings. The protocol relies on Etherfuse as the off-chain custodian of the fund's working capital.

```
USDC (Stellar) ──► Etherfuse ──► TESOURO (held by Etherfuse on classic_wallet)
                          ◄────── yield (BRL/USDC reported back, recorded via add_yield)
```

### Where Etherfuse appears in the protocol

| Flow | Contract call | Off-chain action |
|---|---|---|
| Partner payment ingress | `receive_payment` routes the AUM portion to `classic_wallet` | Etherfuse converts to TESOURO |
| Investor deposit | `deposit_investor` routes USDC to `classic_wallet` | Etherfuse converts to TESOURO |
| Yield reporting | (none; off-chain trigger) | Etherfuse → API call → **Convex Action** → `add_yield(amount)` |
| Off-ramp (redemption) | `process_redemptions` returns total USDC needed | **Convex Action** triggers Etherfuse liquidation → USDC arrives at the operator address (KMS-signed) → Action deposits to contract → `fulfill_redemption` per investor |
| Mgmt fee payout | `charge_mgmt_fee` debits AUM | **Convex Action** sends a Stellar Classic payment (TESOURO asset, PIX MEMO) → Etherfuse → BRL via PIX |

### Known seams / TBD

- The Etherfuse API integration is **not wired in code** in any consumer. Operator runtime moves to `mutav-app` (Convex Actions) per [`#57`](https://github.com/mutav-finance/mutav-stellar/issues/57); the integration design (auth method, retry/idempotency, webhook vs poll) becomes part of the `mutav-app` planning effort ([`mutav-app#139`](https://github.com/mutav-finance/mutav-app/issues/139)). Wallet addresses, asset codes (TESOURO), and PIX MEMO format remain placeholders pending Etherfuse contract negotiation.
- `MEMO` is 28 bytes — UUID and email PIX keys overflow (PR #26 review).
- Yield-reporting cadence and authoritative source (Etherfuse API vs ledger snapshot) are not documented.
- Counterparty risk: there is no on-chain proof that Etherfuse holds the TESOURO it claims to. Mitigation is regulatory / contractual, out of scope here.

## PIX rail

PIX is the Brazilian instant-payment system. Used for the off-chain BRL leg of the management fee payout (and potentially partner-facing payouts in the cover-default path).

### Where PIX appears

- **Mgmt fee**: the operator Convex Action sends a Stellar Classic payment of TESOURO to an Etherfuse-controlled wallet, with the PIX key as a text MEMO. Etherfuse settles the PIX leg to the recipient bank account.
- **Off-ramp** (future): conceptually similar for converting redemption proceeds USDC → BRL.
- **Cover default** (future): partner agencies may be made whole via PIX after a sinistro; today, `cover_default` only debits AUM with no on-chain destination enforcement.

### Format constraints

- `Memo.text` max 28 bytes UTF-8. CPF (11) and CNPJ (14) PIX keys fit; **UUID (36) and email (up to 77) PIX keys do not**. The mgmt-fee Action must validate or hash-encode the key before submission.
- Atomicity gap: on-chain `charge_mgmt_fee` debits AUM *before* the off-chain Classic payment is submitted. If Classic fails, AUM is debited but no fee was sent. The Convex Workflow runtime (Convex's durability primitive) should be used to make the two-step atomic; absent that, manual reconciliation is the only path.

## Partner Horizon accounts

Partner agencies (`imobiliárias`) hold Stellar Classic accounts that pay USDC monthly to the MUTAV operator address. The on-ramp Convex Action reads these payments via Horizon and records them on-chain via `receive_payment`.

### How identity binds

- Partner identity = Stellar G-address.
- Admin maintains a per-fund whitelist via `set_approved_partner(addr, true)`.
- `receive_payment` rejects payments from non-whitelisted partners.

### Reallocation between funds

When an agency's risk profile shifts to a different tier, admin removes them from the current fund and adds to the matching one. Existing contracts already being covered by the old fund continue until expiry (per `contract-introduction.md`).

## RPC provider (testnet/mainnet)

| Network | Soroban RPC | Horizon |
|---|---|---|
| testnet | `soroban-testnet.stellar.org` | `horizon-testnet.stellar.org` |
| mainnet | `mainnet.stellar.validationcloud.io/v1/soroban/rpc` | `horizon.stellar.org` |

Single-vendor on mainnet. The operator runtime (Convex Actions on `mutav-app`) relies on RPC liveness; an outage at validationcloud stalls every Action (heartbeat, on-ramp, off-ramp). Multi-vendor fallback is not implemented (issue #44).

## Known gaps

- Etherfuse API integration is unwritten in any consumer — design lands as part of the operator-runtime move to `mutav-app` ([`mutav-app#139`](https://github.com/mutav-finance/mutav-app/issues/139)).
- PIX MEMO validation is missing.
- Atomic split between on-chain debit and off-chain payout — needs Convex Workflow durability for the mgmt-fee Action; manual reconciliation otherwise.
- Single-RPC dependency on mainnet — issue #44.
- No counterparty-risk documentation for Etherfuse — out of scope here, but should live in the threat model (#46).
