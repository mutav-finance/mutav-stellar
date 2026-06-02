import { describe, expect, test } from "bun:test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { loadDotenv, loadFundEnv, loadTestnetDeploy, requireSecret } from "../env.ts";

function writeEnv(content: string): string {
  const dir = mkdtempSync(join(tmpdir(), "mutav-env-"));
  const path = join(dir, ".env");
  writeFileSync(path, content);
  return path;
}

const MINIMAL_ENV = [
  "STELLAR_NETWORK=testnet",
  "ADMIN_PUBLIC=G_ADMIN",
  "OPERATOR_PUBLIC=G_OP",
  "TREASURY_PUBLIC=G_TR",
  "CLASSIC_WALLET_PUBLIC=G_CW",
  "USDC_TOKEN=C_USDC",
].join("\n");

const FULL_ENV = [
  MINIMAL_ENV,
  "ADMIN_SECRET=S_ADMIN",
  "OPERATOR_SECRET=S_OP",
  "CLASSIC_WALLET_SECRET=S_CW",
].join("\n");

describe("loadDotenv", () => {
  test("parses key=value pairs and ignores comments + blanks", () => {
    const path = writeEnv("FOO=bar\n# comment\n\nBAZ=qux\n");
    expect(loadDotenv(path)).toEqual({ FOO: "bar", BAZ: "qux" });
  });

  test("strips wrapping double + single quotes", () => {
    const path = writeEnv('NAME="MUTAV Fund"\nSYMBOL=\'MTV\'\n');
    expect(loadDotenv(path)).toEqual({ NAME: "MUTAV Fund", SYMBOL: "MTV" });
  });

  test("strips trailing inline comments", () => {
    const path = writeEnv("EXIT_CAP_BPS=1000  # 10% per week\n");
    expect(loadDotenv(path)).toEqual({ EXIT_CAP_BPS: "1000" });
  });

  test("throws when file is missing", () => {
    expect(() => loadDotenv("/nonexistent/local.env")).toThrow(/missing/);
  });
});

describe("loadFundEnv", () => {
  test("loads with only public addresses + USDC token (no secrets needed)", () => {
    const path = writeEnv(MINIMAL_ENV);
    const env = loadFundEnv(path);
    expect(env.network).toBe("testnet");
    expect(env.roles.adminPublic).toBe("G_ADMIN");
    expect(env.roles.operatorPublic).toBe("G_OP");
    expect(env.roles.treasuryPublic).toBe("G_TR");
    expect(env.roles.classicWalletPublic).toBe("G_CW");
    expect(env.usdcToken).toBe("C_USDC");
    // secrets default to undefined when not provided
    expect(env.roles.adminSecret).toBeUndefined();
    expect(env.roles.operatorSecret).toBeUndefined();
    expect(env.roles.classicWalletSecret).toBeUndefined();
  });

  test("populates optional secrets when provided", () => {
    const path = writeEnv(FULL_ENV);
    const env = loadFundEnv(path);
    expect(env.roles.adminSecret).toBe("S_ADMIN");
    expect(env.roles.operatorSecret).toBe("S_OP");
    expect(env.roles.classicWalletSecret).toBe("S_CW");
  });

  test("requires public addresses + USDC token, not signing secrets", () => {
    const path = writeEnv("ADMIN_PUBLIC=G_ADMIN\n");
    expect(() => loadFundEnv(path)).toThrow(/OPERATOR_PUBLIC/);
    expect(() => loadFundEnv(path)).toThrow(/USDC_TOKEN/);
  });

  test("does not require TREASURY_SECRET (treasury never signs)", () => {
    // Sanity: the minimal env above has no TREASURY_SECRET and still loads.
    const path = writeEnv(MINIMAL_ENV);
    expect(() => loadFundEnv(path)).not.toThrow();
  });

  test("applies init-param defaults from local.env.example", () => {
    const path = writeEnv(FULL_ENV);
    const env = loadFundEnv(path);
    expect(env.initParams.tokenName).toBe("MUTAV Fund");
    expect(env.initParams.tokenSymbol).toBe("MTV");
    expect(env.initParams.exitCapBps).toBe(1000);
    expect(env.initParams.mgmtFeeBps).toBe(100);
    expect(env.initParams.redemptionFeeBps).toBe(25);
    expect(env.initParams.protocolFeeBps).toBe(2000);
    expect(env.initParams.fulfillWindowSeconds).toBe(604_800n);
    expect(env.initParams.maxAumIncreaseBps).toBe(500);
  });

  test("overrides init-param defaults when keys are set", () => {
    const path = writeEnv(`${FULL_ENV}\nEXIT_CAP_BPS=2500\nFULFILL_WINDOW_SECONDS=86400\n`);
    const env = loadFundEnv(path);
    expect(env.initParams.exitCapBps).toBe(2500);
    expect(env.initParams.fulfillWindowSeconds).toBe(86_400n);
  });
});

describe("requireSecret", () => {
  test("returns the value when present", () => {
    expect(requireSecret("ADMIN_SECRET", "S_X")).toBe("S_X");
  });

  test("throws a uniform error when undefined", () => {
    expect(() => requireSecret("OPERATOR_SECRET", undefined)).toThrow(
      /OPERATOR_SECRET not set in local.env/,
    );
  });

  test("throws when value is empty string", () => {
    expect(() => requireSecret("ADMIN_SECRET", "")).toThrow(/ADMIN_SECRET/);
  });
});

describe("loadTestnetDeploy", () => {
  test("parses a deploy manifest", () => {
    const dir = mkdtempSync(join(tmpdir(), "mutav-deploy-"));
    const path = join(dir, "testnet.json");
    writeFileSync(
      path,
      JSON.stringify({
        network: "testnet",
        contractId: "C_FUND",
        wasmHash: "abc123",
        deployedAt: "2026-06-01T00:00:00Z",
        rpcUrl: "https://soroban-testnet.stellar.org",
        passphrase: "Test SDF Network ; September 2015",
        initArgs: {
          admin: "G_ADMIN",
          operator: "G_OP",
          protocolAddr: "G_TR",
          usdcToken: "C_USDC",
          classicWallet: "G_CW",
          tokenName: "MUTAV Fund",
          tokenSymbol: "MTV",
          exitCapBps: 1000,
          mgmtFeeBps: 100,
          redemptionFeeBps: 25,
          protocolFeeBps: 2000,
          fulfillWindowSeconds: "604800",
          maxAumIncreaseBps: 500,
        },
      }),
    );
    const d = loadTestnetDeploy(path);
    expect(d.contractId).toBe("C_FUND");
    expect(d.initArgs.protocolAddr).toBe("G_TR");
  });

  test("throws with bootstrap hint when missing", () => {
    expect(() => loadTestnetDeploy("/nonexistent/.stellar/testnet.json")).toThrow(
      /bootstrap:testnet/,
    );
  });
});
