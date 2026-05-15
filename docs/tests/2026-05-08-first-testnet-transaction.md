# First Stellar Testnet Transaction

> **Disposable wallets.** Both keypairs documented here are throwaway testnet
> identities. They MUST NOT be funded on mainnet, reused for any production
> purpose, used to receive real value, or referenced as identity anchors. If
> the secrets ever need to leave `local.env` for any reason, rotate
> immediately: generate fresh keys, sweep any remaining testnet balance, and
> overwrite this document.

**Date:** 2026-05-08
**Network:** Stellar Testnet (`Test SDF Network ; September 2015`)
**Tooling:** `stellar-cli` 26.0.0 (installed via `brew install stellar-cli`)
**Lifetime:** ephemeral — burn and regenerate on demand

## Goal

Smoke-test the Stellar developer toolchain end-to-end before any contract or
TypeScript-package work lands: install the CLI, generate two ephemeral testnet
wallets, fund them via Friendbot, send a native-asset payment between them, and
verify on the public ledger.

This is a manual one-shot test — no automation, no fixtures committed.

## Wallets

Two **disposable, testnet-only** keypairs were generated on the local machine
via the Stellar CLI's HD-seed generator. Public keys only are recorded here.
Secrets and the corresponding 24-word seed phrases live exclusively in
`mutav-stellar/local.env` (gitignored).

These accounts exist solely to validate the developer toolchain. They have no
relationship to any real user, no association with the protocol's production
identity, and no expectation of persistence — assume they may be rotated or
abandoned at any time without notice.

| Alias | Public key                                                 |
| ----- | ---------------------------------------------------------- |
| Alice | `GC5PWPWH7NK4WTDWM3WGUZF2AHSR6SM53T5HUNSZZKYUE34ILFTM6KQI` |
| Bob   | `GCRXIFBQQ2EQYUGPD6R2377BKUIXCZVFZNKSH46TCLFDTM73VOXFZHE3` |

## Commands

Run from `mutav-stellar/` after `brew install stellar-cli`.

```bash
# 1. Configure the testnet network alias
stellar network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"

# 2. Generate two ephemeral keypairs
stellar keys generate alice
stellar keys generate bob

# 3. Fund each via Friendbot (10,000 XLM)
stellar keys fund alice --network testnet
stellar keys fund bob   --network testnet

# 4. Send 10 XLM (= 100_000_000 stroops) Alice -> Bob
stellar tx new payment \
  --network testnet \
  --source alice \
  --destination "$(stellar keys address bob)" \
  --amount 100000000
```

Stroop conversion: 1 XLM = 10,000,000 stroops. The `--amount` flag takes
stroops, not XLM.

## Receipt

```
Operation:      payment
Asset:          native (XLM)
Amount:         10.0000000 XLM
Fee:            100 stroops (0.0000100 XLM)
Ledger:         2456008
Timestamp:      2026-05-09T00:55:12Z
Status:         successful
Tx hash:        8827f8e2455315142b4d3b7e0f86b28a500ced5cebe4f92e070a90cf689a5a93
```

Balances after:

| Account | Balance (XLM) |
| ------- | ------------- |
| Alice   | 9989.9999900  |
| Bob     | 10010.0000000 |

The 0.00001 XLM delta on Alice's side is the network fee.

## Verification

Same data, three views — Horizon JSON is the canonical ledger source; the two
explorers are convenience views and may lag indexing by a few minutes on
testnet.

- **Horizon (raw JSON):**
  https://horizon-testnet.stellar.org/transactions/8827f8e2455315142b4d3b7e0f86b28a500ced5cebe4f92e070a90cf689a5a93
- **Stellar Lab:**
  https://lab.stellar.org/explorer/testnet/transactions/8827f8e2455315142b4d3b7e0f86b28a500ced5cebe4f92e070a90cf689a5a93
- **Stellar Expert:**
  https://stellar.expert/explorer/testnet/tx/8827f8e2455315142b4d3b7e0f86b28a500ced5cebe4f92e070a90cf689a5a93

Account explorers:

- Alice: https://stellar.expert/explorer/testnet/account/GC5PWPWH7NK4WTDWM3WGUZF2AHSR6SM53T5HUNSZZKYUE34ILFTM6KQI
- Bob: https://stellar.expert/explorer/testnet/account/GCRXIFBQQ2EQYUGPD6R2377BKUIXCZVFZNKSH46TCLFDTM73VOXFZHE3

## Secret hygiene

By default, `stellar keys generate` writes the seed phrase to
`~/.config/stellar/identity/<alias>.toml`. After the test, those TOML files
were deleted so `local.env` is the single source of truth for these test
secrets:

```bash
rm ~/.config/stellar/identity/alice.toml ~/.config/stellar/identity/bob.toml
```

`local.env` (in repo root) is gitignored; an explicit `local.env` line was
added to `.gitignore` because the existing `.env.*` glob does not match files
without a leading dot.

Consequence: the `alice` and `bob` aliases no longer resolve via the CLI.
Future `stellar` commands using these accounts must either pass the secret
explicitly (`--source <SECRET>`) or re-import via
`stellar keys add <name> --secret-key` reading from `local.env`.

## Notes for the next test

- These are throwaway testnet keys. Do not fund them on mainnet, do not reuse
  the addresses for anything observable.
- Friendbot rate-limits per-IP. If `stellar keys fund` returns an error,
  wait or use a different network egress.
- The Soroban RPC URL (`https://soroban-testnet.stellar.org`) is required for
  contract operations; for plain payments, Horizon
  (`https://horizon-testnet.stellar.org`) is sufficient.
- When the TypeScript SDK package (`@mutav-finance/mutav-stellar`, Phase A)
  lands, replicate this flow as a programmatic test using
  `@stellar/stellar-sdk` so it can run in CI against testnet on demand.
