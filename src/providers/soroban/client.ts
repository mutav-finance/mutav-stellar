import { rpc } from "@stellar/stellar-sdk";
import type { NetworkConfig } from "../../core/network.ts";

const _servers = new Map<string, rpc.Server>();

export function sorobanClient(net: NetworkConfig): rpc.Server {
  let server = _servers.get(net.rpcUrl);
  if (!server) {
    server = new rpc.Server(net.rpcUrl, { allowHttp: false });
    _servers.set(net.rpcUrl, server);
  }
  return server;
}
