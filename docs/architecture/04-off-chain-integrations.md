# 04 — Off-chain integrations

Three trust boundaries that the on-chain contract cannot enforce: Etherfuse (reserve provider), PIX (BRL rail), and partner Horizon accounts (USDC inflow). Plus the RPC vendor for daemons.

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
| Yield reporting | (none; off-chain trigger) | Etherfuse → API call → daemon → `add_yield(amount)` |
| Off-ramp (redemption) | `process_redemptions` returns total USDC needed | Operator triggers Etherfuse liquidation → USDC arrives on operator wallet → operator deposits to contract → `fulfill_redemption` per investor |
| Mgmt fee payout | `charge_mgmt_fee` debits AUM | Operator sends a Stellar Classic payment (TESOURO asset, PIX MEMO) → Etherfuse → BRL via PIX |

### Known seams / TBD

- The Etherfuse API integration is **not wired in code** — every daemon has `TODO(Etherfuse)` markers. Wallet addresses, asset codes (TESOURO), PIX MEMO format are placeholders today.
- `MEMO` is 28 bytes — UUID and email PIX keys overflow (PR #26 review).
- Yield-reporting cadence and authoritative source (Etherfuse API vs ledger snapshot) are not documented.
- Counterparty risk: there is no on-chain proof that Etherfuse holds the TESOURO it claims to. Mitigation is regulatory / contractual, out of scope here.

## PIX rail

PIX is the Brazilian instant-payment system. Used for the off-chain BRL leg of the management fee payout (and potentially partner-facing payouts in the cover-default path).

### Where PIX appears

- **Mgmt fee** (PR #26): operator sends a Stellar Classic payment of TESOURO to an Etherfuse-controlled wallet, with the PIX key as a text MEMO. Etherfuse settles the PIX leg to the recipient bank account.
- **Off-ramp** (future): conceptually similar for converting redemption proceeds USDC → BRL.
- **Cover default** (future): partner agencies may be made whole via PIX after a sinistro; today, `cover_default` only debits AUM with no on-chain destination enforcement.

### Format constraints

- `Memo.text` max 28 bytes UTF-8. CPF (11) and CNPJ (14) PIX keys fit; **UUID (36) and email (up to 77) PIX keys do not**. The mgmt-fee daemon must validate or hash-encode (PR #26 review).
- Atomicity gap: on-chain `charge_mgmt_fee` debits AUM *before* the off-chain Classic payment is submitted. If Classic fails, AUM is debited but no fee was sent — manual reconciliation only (issue: same as PR #26 review, atomic split).

## Partner Horizon accounts

Partner agencies (`imobiliárias`) hold Stellar Classic accounts that pay USDC monthly to MUTAV's operator wallet. The on-ramp daemon (PR #22) reads these payments via Horizon and records them on-chain via `receive_payment`.

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

Single-vendor on mainnet. Daemons rely on RPC liveness; an outage at validationcloud stalls every daemon (heartbeat, on-ramp, off-ramp). Multi-vendor fallback is not implemented (issue #44).

## Known gaps

- Etherfuse API integration is unwritten (markers in PRs #22, #23, #26).
- PIX MEMO validation is missing (PR #26 review).
- Atomic split between on-chain debit and off-chain payout — no reconciliation layer (PR #26 review + issue #44).
- Single-RPC dependency on mainnet — issue #44.
- No counterparty-risk documentation for Etherfuse — out of scope here, but should live in the threat model (#46).
