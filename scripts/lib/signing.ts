// Sign-and-submit helper shared by every authority-bearing CLI (#69 / #70 / #71).
//
// Pipeline: build TransactionBuilder around the given op → server.prepareTransaction
// (Soroban simulation + footprint + auth) → sign with provided Keypair → submit →
// poll getTransaction until SUCCESS / FAILED / timeout.
//
// This is the only place that ever calls `prepareTransaction` + `sendTransaction`.
// Read-only CLIs (#68) should not touch this module — they go via simulateTransaction
// directly.

import {
  BASE_FEE,
  type Keypair,
  TransactionBuilder,
  rpc,
  type xdr,
} from "@stellar/stellar-sdk";

import type { NetworkConfig } from "../../src/core/network.ts";

export interface SubmitOptions {
  /** Stellar tx timeout in seconds (default 60). */
  timeoutSeconds?: number;
  /** How long to poll getTransaction before giving up (default 30s). */
  pollTimeoutMs?: number;
  /** Polling interval (default 1s). */
  pollIntervalMs?: number;
  /** Fee multiplier — bumps BASE_FEE for congested ledgers (default 1). */
  feeMultiplier?: number;
}

export interface SubmitResult {
  hash: string;
  ledger: number;
  returnValue: xdr.ScVal | undefined;
}

export async function signAndSubmit(
  server: rpc.Server,
  net: NetworkConfig,
  signer: Keypair,
  op: xdr.Operation,
  options: SubmitOptions = {},
): Promise<SubmitResult> {
  const {
    timeoutSeconds = 60,
    pollTimeoutMs = 30_000,
    pollIntervalMs = 1_000,
    feeMultiplier = 1,
  } = options;

  const account = await server.getAccount(signer.publicKey());
  const fee = (BigInt(BASE_FEE) * BigInt(feeMultiplier)).toString();
  const tx = new TransactionBuilder(account, {
    fee,
    networkPassphrase: net.passphrase,
  })
    .addOperation(op)
    .setTimeout(timeoutSeconds)
    .build();

  const prepared = await server.prepareTransaction(tx);
  prepared.sign(signer);

  const send = await server.sendTransaction(prepared);
  if (send.status !== "PENDING") {
    const detail = send.errorResult ? JSON.stringify(send.errorResult) : "(no errorResult)";
    throw new Error(`submit ${send.hash} → ${send.status}: ${detail}`);
  }

  const deadline = Date.now() + pollTimeoutMs;
  while (Date.now() < deadline) {
    const r = await server.getTransaction(send.hash);
    if (r.status === "SUCCESS") {
      return {
        hash: send.hash,
        ledger: r.ledger,
        returnValue: r.returnValue,
      };
    }
    if (r.status === "FAILED") {
      throw new Error(`tx ${send.hash} FAILED: ${JSON.stringify(r.resultXdr)}`);
    }
    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  }
  throw new Error(`tx ${send.hash} not SUCCESS within ${pollTimeoutMs}ms`);
}
