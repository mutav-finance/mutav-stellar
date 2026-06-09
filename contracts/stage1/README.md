# Stage 1 — pilot contracts

The on-chain surface for the pilot stage described in [`mutav-whitepaper.en.md` §5](../../../mutav/docs/mutav-whitepaper.en.md). Stage 1 posture: **Mutav is the *fiador institucional* under art. 37 II of Lei 8.245/91, writing *fiança onerosa* on its own balance sheet** (see also `[[project-stage1-fiador-pivot]]`). The reserve that backs every live guarantee is capitalized by the current pre-seed capital raise, held on-chain in Etherfuse TESOURO, and publicly verifiable from chain state.

## Layout

| Path | Whitepaper § | Role | Code shape |
| --- | --- | --- | --- |
| `reserve_vault/` | §5.4 | Custodial safe holding TESOURO + USDC sleeve. Admin-gated `withdraw(asset, amount, destination, ref_hash)` is the single value-flow path; asset and destination allowlists are admin-managed. All policy (per-asset caps, timelocks, signer thresholds) lives in the OZ Smart Account that holds the `admin` authority. Per-asset `balance(asset)` view enables publishable adequacy checks off-chain. | Soroban Rust crate (audited surface). |
| `collateral_token/` | §5.2, §11h | `MUTAV-COL` — per-lease representative collateral record. **Classic Stellar asset wrapped by SAC** with `AUTH_REQUIRED` / `AUTH_REVOCABLE` / `AUTH_CLAWBACK_ENABLED` + SEP-8 approval server. **Strictly representative — no yield, no rebase, no redemption to a varying amount.** | Issuance + control scripts (no contract code; SAC ships the controls). |

### `reserve_vault/` entry points

| Group | Function | Auth |
| --- | --- | --- |
| Init | `initialize(admin)` | One-shot, no auth check (see deploy notes). |
| Value flow | `withdraw(asset, amount, destination, ref_hash)` | `admin` |
| Allowlists | `add_approved_asset` / `remove_approved_asset` / `add_allowed_destination` / `remove_allowed_destination` | `admin` |
| Governance | `set_paused(paused)` (value-flow pause), `propose_admin(new)` / `accept_admin()` | `admin` / new admin |
| Maintenance | `extend_ttl()` | Permissionless |
| Views | `admin`, `pending_admin`, `paused`, `approved_assets`, `is_approved_asset`, `allowed_destinations`, `is_destination_allowed`, `balance(asset)` | Read-only |

Caps: `MAX_APPROVED_ASSETS = 8`, `MAX_ALLOWED_DESTINATIONS = 64`. Full design: [`docs/specs/2026-06-08-stage1-reserve-vault-design.md`](../../docs/specs/2026-06-08-stage1-reserve-vault-design.md).

## Out of scope for Stage 1

- Investor deposits and quota issuance — that is Stage 2 (§6, §7), implemented under `../stage2/`.
- Any yield mechanic on `MUTAV-COL` — re-classifies the token as *valor mobiliário* under CVM Parecer 40/2022 (§5.2).
- Off-chain operator-key custody — lives on `mutav-app`'s KMS-backed Convex Actions per the boundary rule in [`AGENTS.md`](../../AGENTS.md) and [`CLAUDE.md`](../../CLAUDE.md).

## Stage 1 open questions (from whitepaper §11)

- §11b — `classic_wallet` keypair custody location.
- §11c — over-collateralization ratio target (gated on counsel + actuarial input).
- §11h — collateral-token granularity / holder / backing semantics.
