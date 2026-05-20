import {
  Account,
  Contract,
  TransactionBuilder,
  BASE_FEE,
  nativeToScVal,
  scValToNative,
  Address,
  xdr,
  type Keypair,
  rpc,
} from "@stellar/stellar-sdk";
import type { NetworkConfig } from "../../core/network.ts";
import { sorobanClient } from "./client.ts";

async function invoke(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string,
  method: string,
  args: xdr.ScVal[]
): Promise<rpc.Api.GetTransactionResponse> {
  const server = sorobanClient(net);
  const account = await server.getAccount(operator.publicKey());
  const contract = new Contract(contractId);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: net.passphrase,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(30)
    .build();

  const prepared = await server.prepareTransaction(tx);
  prepared.sign(operator);

  const sent = await server.sendTransaction(prepared);
  if (sent.status === "ERROR") {
    throw new Error(`Transação rejeitada: ${JSON.stringify(sent.errorResult)}`);
  }

  // polling até confirmar
  let result = await server.getTransaction(sent.hash);
  while (result.status === "NOT_FOUND") {
    await Bun.sleep(2000);
    result = await server.getTransaction(sent.hash);
  }

  if (result.status === "FAILED") {
    throw new Error(`Transação falhou: ${sent.hash}`);
  }

  return result;
}

async function query(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string,
  method: string,
  args: xdr.ScVal[] = [],
): Promise<xdr.ScVal> {
  const server = sorobanClient(net);
  // Simulation does not validate sequence numbers — use a placeholder to
  // avoid a getAccount round-trip that would be wasted on a read-only call.
  const account = new Account(operator.publicKey(), "0");
  const contract = new Contract(contractId);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: net.passphrase,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(30)
    .build();

  const simulation = await server.simulateTransaction(tx);

  if (rpc.Api.isSimulationError(simulation)) {
    throw new Error(`Query '${method}' falhou: ${simulation.error}`);
  }

  if (!simulation.result) {
    throw new Error(`Query '${method}': resultado inesperado — entrada expirada precisa de restore?`);
  }

  return simulation.result.retval;
}

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

export async function processRedemptions(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string
): Promise<void> {
  await invoke(net, operator, contractId, "process_redemptions", []);
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

export async function queryAum(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string,
): Promise<bigint> {
  return scValToNative(await query(net, operator, contractId, "aum")) as bigint;
}

export async function queryMaxAumIncreaseBps(
  net: NetworkConfig,
  operator: Keypair,
  contractId: string,
): Promise<number> {
  return scValToNative(await query(net, operator, contractId, "max_aum_increase_bps")) as number;
}
