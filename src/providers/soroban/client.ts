import { rpc } from "@stellar/stellar-sdk";
import type { NetworkConfig } from "../../core/network.ts";

export function sorobanClient(net: NetworkConfig): rpc.Server {
  return new rpc.Server(net.rpcUrl, { allowHttp: false });
}
