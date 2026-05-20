import {
  nativeToScVal,
  scValToNative,
  Address,
  type Keypair,
  rpc,
} from "@stellar/stellar-sdk";
import type { NetworkConfig } from "../../core/network.ts";
import { invoke } from "./invoke.ts";

const READY_REDEMPTION_TOPIC = "rdy_rdmpt";

export async function receivePayment(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string,
  imobiliaria: string,
  amountUsdc: bigint
): Promise<void> {
  await invoke(net, operator, contractId, "receive_payment", [
    new Address(imobiliaria).toScVal(),
    nativeToScVal(amountUsdc, { type: "i128" }),
  ]);
}

export async function addYield(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string,
  amountUsdc: bigint
): Promise<void> {
  await invoke(net, operator, contractId, "add_yield", [
    nativeToScVal(amountUsdc, { type: "i128" }),
  ]);
}

export async function chargeMgmtFee(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string
): Promise<void> {
  await invoke(net, operator, contractId, "charge_mgmt_fee", []);
}

export interface ReadyRedemptionsResult {
  totalUsdc: bigint;
  investors: string[];
}

function extractReadyInvestors(result: rpc.Api.GetTransactionResponse): string[] {
  if (result.status !== "SUCCESS") return [];
  const events = result.resultMetaXdr.v3()?.sorobanMeta()?.events() ?? [];
  const investors: string[] = [];
  for (const event of events) {
    try {
      const topics = event.body().v0().topics();
      if (topics.length < 2) continue;
      if (scValToNative(topics[0]!) !== READY_REDEMPTION_TOPIC) continue;
      investors.push(Address.fromScVal(topics[1]!).toString());
    } catch {
      // skip malformed event
    }
  }
  return investors;
}

export async function processRedemptions(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string
): Promise<ReadyRedemptionsResult> {
  const result = await invoke(net, operator, contractId, "process_redemptions", []);
  const investors = extractReadyInvestors(result);
  let totalUsdc = 0n;
  if (result.status === "SUCCESS" && result.returnValue != null) {
    totalUsdc = scValToNative(result.returnValue) as bigint;
  }
  return { totalUsdc, investors };
}

export async function fulfillRedemption(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string,
  investor: string
): Promise<void> {
  await invoke(net, operator, contractId, "fulfill_redemption", [
    new Address(investor).toScVal(),
  ]);
}

export async function recordOffchainPayout(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string,
  amountUsdc: bigint,
  destination: string
): Promise<void> {
  await invoke(net, operator, contractId, "record_offchain_payout", [
    nativeToScVal(amountUsdc, { type: "i128" }),
    new Address(destination).toScVal(),
  ]);
}

export async function extendTtl(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string
): Promise<void> {
  await invoke(net, operator, contractId, "extend_ttl", []);
}

export async function extendBalanceTtl(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string,
  investor: string
): Promise<void> {
  await invoke(net, operator, contractId, "extend_balance_ttl", [
    new Address(investor).toScVal(),
  ]);
}
