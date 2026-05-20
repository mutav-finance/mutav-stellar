import {
  Horizon,
  TransactionBuilder,
  BASE_FEE,
  Asset,
  Memo,
  Operation,
  type Keypair,
} from "@stellar/stellar-sdk";
import type { NetworkConfig } from "../../core/network.ts";

const _servers = new Map<string, Horizon.Server>();

function horizonClient(net: NetworkConfig): Horizon.Server {
  let server = _servers.get(net.horizonUrl);
  if (!server) {
    server = new Horizon.Server(net.horizonUrl, { allowHttp: false });
    _servers.set(net.horizonUrl, server);
  }
  return server;
}

export async function sendClassicPayment(
  net: NetworkConfig,
  from: Keypair,
  to: string,
  asset: Asset,
  amount: string,
  memo: string,
): Promise<void> {
  const server = horizonClient(net);
  const account = await server.loadAccount(from.publicKey());

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: net.passphrase,
  })
    .addOperation(Operation.payment({ destination: to, asset, amount }))
    .addMemo(Memo.text(memo))
    .setTimeout(30)
    .build();

  tx.sign(from);

  const result = await server.submitTransaction(tx);
  if (!result.successful) {
    throw new Error(`Pagamento Clássico falhou: ${result.hash}`);
  }
}
