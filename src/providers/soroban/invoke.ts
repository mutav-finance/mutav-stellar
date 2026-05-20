import {
  Contract,
  TransactionBuilder,
  BASE_FEE,
  xdr,
  type Keypair,
  rpc,
} from "@stellar/stellar-sdk";
import type { NetworkConfig } from "../../core/network.ts";
import { sorobanClient } from "./client.ts";

export async function invoke(
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
