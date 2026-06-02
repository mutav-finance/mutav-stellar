#!/usr/bin/env bun
/**
 * Bootstrap a testnet MUTAV Fund instance.
 *
 * First run: builds the wasm, uploads it, deploys the contract, calls
 * `initialize`, writes `.stellar/testnet.json`. Re-runs short-circuit to a
 * verify-alive check on the recorded contract ID.
 *
 * Network passphrase + RPC URL come from `src/core/network.ts`.
 * Roles + init params come from `local.env` (see `local.env.example`).
 *
 *   bun run scripts/bootstrap-testnet.ts
 */

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { Keypair, rpc, xdr } from "@stellar/stellar-sdk";

import { resolveNetwork, type NetworkConfig } from "../src/core/network.ts";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const ENV_FILE = join(ROOT, "local.env");
const TESTNET_JSON = join(ROOT, ".stellar", "testnet.json");
const WASM_PATH = join(ROOT, "target", "wasm32v1-none", "release", "mutav_fund.wasm");
const FRIENDBOT_URL = "https://friendbot.stellar.org";

interface BootstrapEnv {
  adminPublic: string;
  adminSecret: string;
  operatorPublic: string;
  treasuryPublic: string;
  classicWalletPublic: string;
  usdcToken: string;
  tokenName: string;
  tokenSymbol: string;
  exitCapBps: string;
  mgmtFeeBps: string;
  redemptionFeeBps: string;
  protocolFeeBps: string;
  fulfillWindowSeconds: string;
  maxAumIncreaseBps: string;
}

function parseEnvFile(path: string): Record<string, string> {
  const env: Record<string, string> = {};
  for (const raw of readFileSync(path, "utf8").split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq === -1) continue;
    const key = line.slice(0, eq).trim();
    let value = line.slice(eq + 1).trim();
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

function readBootstrapEnv(): BootstrapEnv {
  if (!existsSync(ENV_FILE)) {
    throw new Error(`missing ${ENV_FILE} — copy local.env.example and fill in values`);
  }
  const e = parseEnvFile(ENV_FILE);
  const required = [
    "ADMIN_SECRET",
    "OPERATOR_PUBLIC",
    "TREASURY_PUBLIC",
    "CLASSIC_WALLET_PUBLIC",
    "USDC_TOKEN",
  ];
  const missing = required.filter((k) => !e[k]);
  if (missing.length) throw new Error(`local.env missing: ${missing.join(", ")}`);

  const adminSecret = e.ADMIN_SECRET!;
  const adminPublic = e.ADMIN_PUBLIC || Keypair.fromSecret(adminSecret).publicKey();

  return {
    adminPublic,
    adminSecret,
    operatorPublic: e.OPERATOR_PUBLIC!,
    treasuryPublic: e.TREASURY_PUBLIC!,
    classicWalletPublic: e.CLASSIC_WALLET_PUBLIC!,
    usdcToken: e.USDC_TOKEN!,
    tokenName: e.TOKEN_NAME ?? "MUTAV Fund",
    tokenSymbol: e.TOKEN_SYMBOL ?? "MTV",
    exitCapBps: e.EXIT_CAP_BPS ?? "1000",
    mgmtFeeBps: e.MGMT_FEE_BPS ?? "100",
    redemptionFeeBps: e.REDEMPTION_FEE_BPS ?? "25",
    protocolFeeBps: e.PROTOCOL_FEE_BPS ?? "2000",
    fulfillWindowSeconds: e.FULFILL_WINDOW_SECONDS ?? "604800",
    maxAumIncreaseBps: e.MAX_AUM_INCREASE_BPS ?? "500",
  };
}

async function friendbotFundIfEmpty(publicKey: string, server: rpc.Server): Promise<void> {
  try {
    await server.getAccount(publicKey);
    return;
  } catch {
    // not found — fall through to friendbot
  }
  const res = await fetch(`${FRIENDBOT_URL}/?addr=${encodeURIComponent(publicKey)}`);
  if (!res.ok) {
    throw new Error(`friendbot ${publicKey}: ${res.status} ${await res.text()}`);
  }
  console.log(`  funded ${publicKey}`);
}

function runStellar(args: string[]): string {
  const r = spawnSync("stellar", args, { cwd: ROOT, encoding: "utf8" });
  if (r.status !== 0) {
    process.stderr.write(r.stderr ?? "");
    throw new Error(`stellar ${args.join(" ")} → exit ${r.status}`);
  }
  return (r.stdout ?? "").trim();
}

function runStellarStreaming(args: string[]): void {
  const r = spawnSync("stellar", args, { cwd: ROOT, stdio: "inherit" });
  if (r.status !== 0) throw new Error(`stellar ${args.join(" ")} → exit ${r.status}`);
}

async function isContractAlive(server: rpc.Server, contractId: string): Promise<boolean> {
  try {
    await server.getContractData(
      contractId,
      xdr.ScVal.scvLedgerKeyContractInstance(),
      rpc.Durability.Persistent,
    );
    return true;
  } catch {
    return false;
  }
}

async function main(): Promise<void> {
  const net = resolveNetwork();
  if (net.name !== "testnet") {
    throw new Error(`bootstrap-testnet refuses non-testnet network: ${net.name}`);
  }
  const env = readBootstrapEnv();
  const server = new rpc.Server(net.rpcUrl, { allowHttp: false });

  if (existsSync(TESTNET_JSON)) {
    const existing = JSON.parse(readFileSync(TESTNET_JSON, "utf8")) as { contractId: string };
    console.log(`· found existing deploy: ${existing.contractId}`);
    if (await isContractAlive(server, existing.contractId)) {
      console.log("✓ alive on testnet — nothing to do");
      return;
    }
    console.warn(
      "⚠ recorded contract not alive on RPC.\n" +
        "  Delete .stellar/testnet.json to force re-bootstrap.",
    );
    process.exitCode = 1;
    return;
  }

  console.log(`· bootstrapping ${net.name} → ${net.rpcUrl}`);

  console.log("· funding roles via friendbot");
  for (const pk of [
    env.adminPublic,
    env.operatorPublic,
    env.treasuryPublic,
    env.classicWalletPublic,
  ]) {
    await friendbotFundIfEmpty(pk, server);
  }

  console.log("· building contract wasm");
  runStellarStreaming(["contract", "build"]);
  if (!existsSync(WASM_PATH)) {
    throw new Error(`wasm not produced at ${WASM_PATH}`);
  }

  const networkArgs = (n: NetworkConfig) => [
    "--rpc-url", n.rpcUrl,
    "--network-passphrase", n.passphrase,
  ];

  console.log("· uploading wasm");
  const wasmHash = runStellar([
    "contract", "upload",
    "--wasm", WASM_PATH,
    "--source-account", env.adminSecret,
    ...networkArgs(net),
  ]);
  console.log(`  wasmHash=${wasmHash}`);

  console.log("· deploying contract");
  const contractId = runStellar([
    "contract", "deploy",
    "--wasm-hash", wasmHash,
    "--source-account", env.adminSecret,
    ...networkArgs(net),
  ]);
  console.log(`  contractId=${contractId}`);

  console.log("· initializing");
  runStellarStreaming([
    "contract", "invoke",
    "--id", contractId,
    "--source-account", env.adminSecret,
    ...networkArgs(net),
    "--",
    "initialize",
    "--admin", env.adminPublic,
    "--operator", env.operatorPublic,
    "--protocol_addr", env.treasuryPublic,
    "--usdc_token", env.usdcToken,
    "--classic_wallet", env.classicWalletPublic,
    "--token_name", env.tokenName,
    "--token_symbol", env.tokenSymbol,
    "--exit_cap_bps", env.exitCapBps,
    "--mgmt_fee_bps", env.mgmtFeeBps,
    "--redemption_fee_bps", env.redemptionFeeBps,
    "--protocol_fee_bps", env.protocolFeeBps,
    "--fulfill_window_seconds", env.fulfillWindowSeconds,
    "--max_aum_increase_bps", env.maxAumIncreaseBps,
  ]);

  mkdirSync(dirname(TESTNET_JSON), { recursive: true });
  const manifest = {
    network: "testnet",
    contractId,
    wasmHash,
    deployedAt: new Date().toISOString(),
    rpcUrl: net.rpcUrl,
    passphrase: net.passphrase,
    initArgs: {
      admin: env.adminPublic,
      operator: env.operatorPublic,
      protocolAddr: env.treasuryPublic,
      usdcToken: env.usdcToken,
      classicWallet: env.classicWalletPublic,
      tokenName: env.tokenName,
      tokenSymbol: env.tokenSymbol,
      exitCapBps: Number(env.exitCapBps),
      mgmtFeeBps: Number(env.mgmtFeeBps),
      redemptionFeeBps: Number(env.redemptionFeeBps),
      protocolFeeBps: Number(env.protocolFeeBps),
      fulfillWindowSeconds: env.fulfillWindowSeconds,
      maxAumIncreaseBps: Number(env.maxAumIncreaseBps),
    },
  };
  writeFileSync(TESTNET_JSON, JSON.stringify(manifest, null, 2) + "\n");
  console.log(`✓ wrote ${TESTNET_JSON}`);
}

main().catch((err: unknown) => {
  console.error("✗ bootstrap failed:", err instanceof Error ? err.message : err);
  process.exitCode = 1;
});
