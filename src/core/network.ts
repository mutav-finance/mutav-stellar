import { Networks } from "@stellar/stellar-sdk";

export type NetworkName = "testnet" | "mainnet";

export interface NetworkConfig {
  name: NetworkName;
  rpcUrl: string;
  horizonUrl: string;
  passphrase: string;
}

const NETWORKS: Record<NetworkName, NetworkConfig> = {
  testnet: {
    name: "testnet",
    rpcUrl: "https://soroban-testnet.stellar.org",
    horizonUrl: "https://horizon-testnet.stellar.org",
    passphrase: Networks.TESTNET,
  },
  mainnet: {
    name: "mainnet",
    rpcUrl: "https://mainnet.stellar.validationcloud.io/v1/soroban/rpc",
    horizonUrl: "https://horizon.stellar.org",
    passphrase: Networks.PUBLIC,
  },
};

export function resolveNetwork(override?: NetworkName): NetworkConfig {
  const name =
    override ??
    (process.env.STELLAR_NETWORK as NetworkName | undefined) ??
    "testnet";
  const config = NETWORKS[name];
  if (!config) throw new Error(`Unknown network: ${name}`);
  return config;
}
