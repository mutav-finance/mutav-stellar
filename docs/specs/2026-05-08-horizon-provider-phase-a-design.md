# SGR Stellar API — Phase A: Horizon Provider

**Date:** 2026-05-08
**Repo:** `mutav-finance/mutav-stellar`
**Status:** Approved (Phase A only — Soroban provider is a future spec)

## Context

`mutav-stellar` is the Stellar-side implementation of the SGR (Sistema de Garantia Registrada) rental-guarantee protocol. The repo is currently empty (CLAUDE.md, README.md, brand vendor, githook, design assets).

Within Stellar there are two distinct stacks: **Horizon** (classic accounts, payments, friendbot, ledger queries) and **Soroban** (smart contracts, RPC, contract invocation). This spec organizes `mutav-stellar` to expose both as **two providers** behind one TypeScript API package, `@mutav-finance/mutav-stellar`. `mutav-app` (the dashboard) will eventually consume this package.

This spec covers **Phase A: the Horizon provider**, plus the shared core (network config, keypair management) that the Soroban provider will reuse later. Phase B (Soroban provider + Rust contracts) is a separate spec.

## Goal

Phase A delivers:

1. A network-parametric Stellar API package skeleton (`testnet` default, `mainnet` selectable via env).
2. Shared core utilities: network config and ed25519 keypair management.
3. The **Horizon provider**, exposing four operations: generate keypair, fund via friendbot, query balance, send native XLM payment.
4. A thin CLI that wraps the Horizon provider for manual testing on testnet.

## Non-goals

- No Soroban code (Phase B).
- No Rust contracts in this spec (the `contracts/` directory is reserved for Phase B).
- No multi-keypair management — `.env.local` holds one keypair.
- No asset issuance, trustlines, path payments, or multisig.
- No `mutav-app` wiring — mutav-stellar stays standalone for Phase A; mutav-app picks it up later.
- No automated tests — manual testnet runs are the verification method for Phase A.

## Stack

| Concern    | Choice                                                                  | Reason |
| ---------- | ----------------------------------------------------------------------- | ------ |
| Runtime    | bun                                                                     | Matches sibling `mutav-app`'s `packageManager: bun@1.3.1`. Native TS execution, native `.env.local` loading. |
| SDK        | `@stellar/stellar-sdk`                                                  | Official JS SDK; exposes both `Horizon.Server` and `rpc.Server`, so the same dep covers both providers. |
| Language   | TypeScript, run via `bun run` (no compile step in dev)                  | Lowest friction. Build step (if needed for publish) deferred until mutav-app actually consumes the package. |
| Networks   | Stellar **testnet** (default) and **mainnet** (opt-in via env)          | Testnet on dev, mainnet on prod. `fund` errors on mainnet (friendbot is testnet-only). |

### Network endpoints

| Network | Horizon                                | Passphrase (`Networks.*` constant) | Friendbot                         |
| ------- | -------------------------------------- | ---------------------------------- | --------------------------------- |
| testnet | `https://horizon-testnet.stellar.org`  | `Networks.TESTNET`                 | `https://friendbot.stellar.org`   |
| mainnet | `https://horizon.stellar.org`          | `Networks.PUBLIC`                  | n/a (operation rejected)          |

## Architecture

Two providers behind one package. Phase A implements only Horizon; Soroban arrives in Phase B.

**No barrel files.** Per `CONTRIBUTING.md`, `index.ts` re-export barrels are forbidden. Public API entry points are declared via the `package.json` `exports` map, and consumers import from specific subpaths (e.g. `import { fund } from '@mutav-finance/mutav-stellar/horizon/account'`). Internal modules import from sibling files directly, never via an aggregator.

```
@mutav-finance/mutav-stellar (package)
├── core/                    shared primitives
│   ├── network.ts           NetworkName ('testnet'|'mainnet'), config lookup, env resolver
│   └── wallet.ts            Keypair load/save/validate (.env.local-backed, ed25519)
├── providers/
│   └── horizon/             Phase A — classic Stellar operations
│       ├── client.ts        Horizon.Server factory parameterized by network
│       ├── account.ts       fund (friendbot), balance, sequence
│       └── payment.ts       build/sign/submit native XLM transfer
└── cli/                     thin wrappers for manual testing
    ├── new.ts               → core/wallet.generate
    ├── fund.ts              → providers/horizon/account.fund
    ├── balance.ts           → providers/horizon/account.balance
    └── send.ts              → providers/horizon/payment.send
```

`providers/soroban/` and `contracts/` (Rust) will be added by Phase B as siblings.

## File layout

```
mutav-stellar/
├── package.json              # name: @mutav-finance/mutav-stellar, type: module, exports map (no main barrel)
├── tsconfig.json             # strict, module: ESNext, moduleResolution: bundler, types: bun
├── .env.example              # committed: STELLAR_NETWORK=, STELLAR_SECRET=, STELLAR_PUBLIC=
├── .env.local                # gitignored (already covered by .gitignore)
├── src/
│   ├── core/
│   │   ├── network.ts
│   │   └── wallet.ts
│   ├── providers/
│   │   └── horizon/
│   │       ├── client.ts
│   │       ├── account.ts
│   │       └── payment.ts
│   └── cli/
│       ├── new.ts
│       ├── fund.ts
│       ├── balance.ts
│       └── send.ts
└── README.md                 # updated with API + CLI usage
```

No `index.ts` files anywhere — barrel files are forbidden by `CONTRIBUTING.md`. `.gitignore` already covers `.env*`, `node_modules`, `dist/`. No `.gitignore` change needed.

### `package.json` `exports` map (no barrels)

The package exposes named subpaths for each module. Each `exports` entry points directly at the source file (or its build output, when a build step is added). No aggregator file.

```jsonc
{
  "name": "@mutav-finance/mutav-stellar",
  "type": "module",
  "exports": {
    "./core/network":      "./src/core/network.ts",
    "./core/wallet":       "./src/core/wallet.ts",
    "./horizon/client":    "./src/providers/horizon/client.ts",
    "./horizon/account":   "./src/providers/horizon/account.ts",
    "./horizon/payment":   "./src/providers/horizon/payment.ts"
  }
}
```

Consumers (mutav-app, future code) import from these subpaths:

```ts
import { resolveNetwork }    from '@mutav-finance/mutav-stellar/core/network';
import { sendPayment }       from '@mutav-finance/mutav-stellar/horizon/payment';
```

Phase B adds new entries (`./soroban/client`, `./soroban/contract`, etc.) — never an aggregated `./soroban` barrel.

## Module contracts

### `core/network.ts`

```ts
export type NetworkName = 'testnet' | 'mainnet';

export interface NetworkConfig {
  name: NetworkName;
  horizonUrl: string;
  passphrase: string;     // Networks.TESTNET or Networks.PUBLIC
  friendbotUrl?: string;  // testnet only
}

export const NETWORKS: Record<NetworkName, NetworkConfig>;
export function resolveNetwork(override?: NetworkName): NetworkConfig;
// Resolution order: explicit override > STELLAR_NETWORK env > 'testnet'.
```

### `core/wallet.ts`

```ts
export interface StoredKeypair { publicKey: string; secret: string; }

export function generateKeypair(): StoredKeypair;
export function loadKeypair(): StoredKeypair;             // reads STELLAR_SECRET/PUBLIC
export function saveKeypair(kp: StoredKeypair, opts?: { force?: boolean }): void;
export function validatePublicKey(s: string): boolean;    // StrKey.isValidEd25519PublicKey
```

`saveKeypair` writes/updates `.env.local`, refusing to overwrite an existing `STELLAR_SECRET` unless `opts.force === true`.

### `providers/horizon/client.ts`

```ts
import { Horizon } from '@stellar/stellar-sdk';

export function horizonClient(net: NetworkConfig): Horizon.Server;
```

(Note: in `@stellar/stellar-sdk` v11+ the classic client is `Horizon.Server`, not bare `Server`; bare `Server` belongs to Soroban RPC. The spec uses the v11+ shape.)

### `providers/horizon/account.ts`

```ts
export async function fund(net: NetworkConfig, publicKey: string): Promise<void>;
// throws if net.name === 'mainnet' (no friendbot on mainnet)

export interface AccountSummary { publicKey: string; xlm: string; sequence: string; }
export async function balance(net: NetworkConfig, publicKey: string): Promise<AccountSummary>;
```

### `providers/horizon/payment.ts`

```ts
export interface PaymentArgs {
  net: NetworkConfig;
  sourceSecret: string;
  destination: string;
  amount: string;          // human-readable, e.g. "10" or "0.0000001"
}

export interface PaymentResult { hash: string; explorerUrl: string; }

export async function sendPayment(args: PaymentArgs): Promise<PaymentResult>;
```

Implementation builds a `TransactionBuilder` with `BASE_FEE`, the resolved `passphrase`, a 30-second `setTimeout`, and a single `Operation.payment({ destination, asset: Asset.native(), amount })`. Signs with the source `Keypair`, submits via `horizonClient(net).submitTransaction(tx)`. Returns the tx hash plus a Stellar Expert URL — `https://stellar.expert/explorer/testnet/tx/{hash}` for testnet, `https://stellar.expert/explorer/public/tx/{hash}` for mainnet (Stellar Expert uses the slug `public` for mainnet).

### Public API surface (no barrel)

There is no `src/index.ts`. The package's public surface is the `exports` map above. Each module file is its own entry point; consumers import from `@mutav-finance/mutav-stellar/<subpath>`. Internal cross-module imports inside `src/` reference sibling files directly (e.g. `import { horizonClient } from '../horizon/client'` from `account.ts`), never an aggregator.

Phase B adds new entries to the `exports` map; it does **not** introduce any aggregator files.

## CLI surface

Exposed via `package.json` scripts. Each script reads `STELLAR_NETWORK` from env (default `testnet`), prints the active network on the first line of output, and dispatches to the API.

| Command                                          | File                | Behavior |
| ------------------------------------------------ | ------------------- | -------- |
| `bun run wallet:new`                             | `src/cli/new.ts`    | `wallet.generateKeypair()` + `wallet.saveKeypair()`. Refuses overwrite without `--force`. |
| `bun run wallet:fund`                            | `src/cli/fund.ts`   | `horizon.account.fund(network, public)`. Errors if `STELLAR_NETWORK=mainnet`. |
| `bun run wallet:balance`                         | `src/cli/balance.ts`| `horizon.account.balance(network, public)`. Prints public key, XLM balance, sequence. |
| `bun run wallet:send <destination> <amount>`     | `src/cli/send.ts`   | Validates args, then `horizon.payment.sendPayment(...)`. Prints tx hash + explorer URL. |

## Data flow (CLI level)

### `wallet:new`

1. Parse `--force` from argv.
2. `core.wallet.generateKeypair()` → fresh ed25519 keypair.
3. `core.wallet.saveKeypair(kp, { force })` → writes/replaces `STELLAR_SECRET` and `STELLAR_PUBLIC` in `.env.local`. Without `--force` and an existing secret, exits non-zero with: "`.env.local` already has a keypair. Pass `--force` to overwrite."
4. Print public key + "next: `bun run wallet:fund`".

### `wallet:fund`

1. `core.network.resolveNetwork()` → testnet by default.
2. `core.wallet.loadKeypair()` → friendly error if missing, pointing at `wallet:new`.
3. `horizon.account.fund(network, public)`:
   - If `network.name === 'mainnet'`, throw `MainnetFriendbotError`.
   - Else `GET ${friendbotUrl}?addr=${public}`. On 4xx, throw with the response body (handles "createAccountAlreadyExist").
4. Re-fetch balance, print "funded — XLM balance: <n>".

### `wallet:balance`

1. Resolve network + load keypair.
2. `horizon.account.balance(network, public)` → `Horizon.Server.loadAccount(public)`. On 404, friendly error pointing at `wallet:fund`.
3. Print public key, XLM balance, sequence.

### `wallet:send <destination> <amount>`

1. Validate args:
   - `core.wallet.validatePublicKey(destination)` (uses `StrKey.isValidEd25519PublicKey`).
   - `amount` parses as a positive number with at most 7 decimal places (Stellar's stroop precision).
2. Resolve network + load keypair (need `secret` here).
3. `horizon.payment.sendPayment({ net, sourceSecret: secret, destination, amount })`.
4. Print tx hash + Stellar Expert URL. On Horizon error, log `extras.result_codes` from the response envelope and exit non-zero.

## Error handling principles

- Every CLI command checks env up front and exits with one-line, action-oriented messages ("run `bun run wallet:new`").
- Horizon errors are surfaced via `extras.result_codes`, not raw stack traces.
- `fund` rejects mainnet at the API layer, not the CLI layer — so any future caller (mutav-app) gets the same protection.
- No silent fallbacks — friendbot 4xx fails loudly.

## Security

- `.env.local` is local dev only and gitignored (`.env*` already covered).
- README and `.env.example` will state: never reuse a keypair generated by this tool for mainnet without rotating funds responsibly.
- `STELLAR_NETWORK` defaults to `testnet` — switching to mainnet requires an explicit env change.
- No hardcoded mainnet credentials. Mainnet is just a network selector; secrets still come from `.env.local`.

## README delta

Add to `mutav-stellar/README.md`:

> ## Stellar API
>
> `@mutav-finance/mutav-stellar` exposes two providers for the Stellar stacks:
>
> - **Horizon** (classic accounts, payments, friendbot) — Phase A, available now.
> - **Soroban** (smart contracts) — Phase B, in development.
>
> ### Local CLI (testnet)
>
> ```
> bun install
> bun run wallet:new                           # generate keypair → .env.local
> bun run wallet:fund                          # friendbot funds it (testnet only)
> bun run wallet:balance                       # show XLM balance
> bun run wallet:send G... 10                  # send 10 XLM to a destination
> ```
>
> Set `STELLAR_NETWORK=mainnet` in `.env.local` for mainnet operations. Friendbot is testnet-only and will reject mainnet requests.

The existing protocol-overview content stays untouched.

## CLAUDE.md delta

Phase A implementation should update `mutav-stellar/CLAUDE.md` to reflect:

- **Stack** also includes TypeScript (bun runtime, `@stellar/stellar-sdk`).
- **Code standards** for TS: defer formatter choice until needed (no Biome/Prettier added in Phase A).
- **Layout note:** `src/providers/horizon/` is the Horizon API; `src/providers/soroban/` will be added with Phase B; `contracts/` is reserved for the Rust source.
- **Public API surface** is declared via `package.json` `exports`, not via `src/index.ts` (consistent with the no-barrel rule already in `CONTRIBUTING.md`).

## Open questions

None. Phase B (Soroban provider + Rust contracts) deferred to a separate spec.
