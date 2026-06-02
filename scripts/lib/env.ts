// Typed loader for `local.env` + `.stellar/testnet.json`.
//
// All Phase A CLIs (#68–#71) consume this. Defaults mirror local.env.example.

import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import type { NetworkName } from "../../src/core/network.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "..", "..");
export const ENV_FILE = join(ROOT, "local.env");
export const TESTNET_JSON = join(ROOT, ".stellar", "testnet.json");

// Authorities on the Fund contract.
//
// Secrets are OPTIONAL on the loader — only the CLI that actually needs to sign
// as a given role requires that role's secret. Call `requireSecret` at use
// site. Treasury never signs (passive recipient) — no secret field.
export interface Roles {
  adminPublic: string;
  adminSecret?: string;
  operatorPublic: string;
  operatorSecret?: string;
  treasuryPublic: string;
  classicWalletPublic: string;
  classicWalletSecret?: string; // only for one-off USDC funding on testnet
}

export interface InvestorKeys {
  alicePublic?: string;
  aliceSecret?: string;
  bobPublic?: string;
  bobSecret?: string;
}

export interface InitParams {
  tokenName: string;
  tokenSymbol: string;
  exitCapBps: number;
  mgmtFeeBps: number;
  redemptionFeeBps: number;
  protocolFeeBps: number;
  fulfillWindowSeconds: bigint;
  maxAumIncreaseBps: number;
}

export interface FundEnv {
  network: NetworkName;
  roles: Roles;
  investors: InvestorKeys;
  usdcToken: string;
  initParams: InitParams;
}

export interface TestnetDeploy {
  network: "testnet";
  contractId: string;
  wasmHash: string;
  deployedAt: string;
  rpcUrl: string;
  passphrase: string;
  initArgs: {
    admin: string;
    operator: string;
    protocolAddr: string;
    usdcToken: string;
    classicWallet: string;
    tokenName: string;
    tokenSymbol: string;
    exitCapBps: number;
    mgmtFeeBps: number;
    redemptionFeeBps: number;
    protocolFeeBps: number;
    fulfillWindowSeconds: string;
    maxAumIncreaseBps: number;
  };
}

function parseDotenv(text: string): Record<string, string> {
  const env: Record<string, string> = {};
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq === -1) continue;
    const key = line.slice(0, eq).trim();
    let value = line.slice(eq + 1).trim();
    // strip a trailing inline comment that begins with `  #` (two spaces + hash)
    const commentIdx = value.indexOf("  #");
    if (commentIdx !== -1) value = value.slice(0, commentIdx).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    env[key] = value;
  }
  return env;
}

export function loadDotenv(path: string = ENV_FILE): Record<string, string> {
  if (!existsSync(path)) {
    throw new Error(`missing ${path} — copy local.env.example and fill in values`);
  }
  return parseDotenv(readFileSync(path, "utf8"));
}

// Only the public addresses + the USDC token are required at load time:
// they are wired into the Fund's `initialize` args + into auth checks.
// Signing secrets are role-scoped — see `requireSecret`.
const REQUIRED_KEYS = [
  "ADMIN_PUBLIC",
  "OPERATOR_PUBLIC",
  "TREASURY_PUBLIC",
  "CLASSIC_WALLET_PUBLIC",
  "USDC_TOKEN",
] as const;

export function loadFundEnv(path: string = ENV_FILE): FundEnv {
  const e = loadDotenv(path);
  const missing = REQUIRED_KEYS.filter((k) => !e[k]);
  if (missing.length) {
    throw new Error(`local.env missing: ${missing.join(", ")}`);
  }
  const network = (e.STELLAR_NETWORK ?? "testnet") as NetworkName;
  return {
    network,
    roles: {
      adminPublic: e.ADMIN_PUBLIC!,
      adminSecret: e.ADMIN_SECRET || undefined,
      operatorPublic: e.OPERATOR_PUBLIC!,
      operatorSecret: e.OPERATOR_SECRET || undefined,
      treasuryPublic: e.TREASURY_PUBLIC!,
      classicWalletPublic: e.CLASSIC_WALLET_PUBLIC!,
      classicWalletSecret: e.CLASSIC_WALLET_SECRET || undefined,
    },
    investors: {
      alicePublic: e.ALICE_PUBLIC,
      aliceSecret: e.ALICE_SECRET,
      bobPublic: e.BOB_PUBLIC,
      bobSecret: e.BOB_SECRET,
    },
    usdcToken: e.USDC_TOKEN!,
    initParams: {
      tokenName: e.TOKEN_NAME ?? "MUTAV Fund",
      tokenSymbol: e.TOKEN_SYMBOL ?? "MTV",
      exitCapBps: Number(e.EXIT_CAP_BPS ?? "1000"),
      mgmtFeeBps: Number(e.MGMT_FEE_BPS ?? "100"),
      redemptionFeeBps: Number(e.REDEMPTION_FEE_BPS ?? "25"),
      protocolFeeBps: Number(e.PROTOCOL_FEE_BPS ?? "2000"),
      fulfillWindowSeconds: BigInt(e.FULFILL_WINDOW_SECONDS ?? "604800"),
      maxAumIncreaseBps: Number(e.MAX_AUM_INCREASE_BPS ?? "500"),
    },
  };
}

/**
 * Throw a uniform "missing secret" error.
 *
 *   const secret = requireSecret("OPERATOR_SECRET", env.roles.operatorSecret);
 *
 * Use at the CLI entrypoint that actually signs — keeps the env loader
 * agnostic about which authority a given command exercises.
 */
export function requireSecret(envKey: string, value: string | undefined): string {
  if (!value) {
    throw new Error(`${envKey} not set in local.env — required for this command`);
  }
  return value;
}

export function loadTestnetDeploy(path: string = TESTNET_JSON): TestnetDeploy {
  if (!existsSync(path)) {
    throw new Error(`missing ${path} — run \`bun run bootstrap:testnet\` first`);
  }
  return JSON.parse(readFileSync(path, "utf8")) as TestnetDeploy;
}
