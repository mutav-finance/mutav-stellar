import {
  nativeToScVal,
  Address,
  type Keypair,
} from "@stellar/stellar-sdk";
import type { NetworkConfig } from "../../core/network.ts";
import { invoke } from "./invoke.ts";

export async function transferToken(
  net: NetworkConfig,
  from: Keypair,
  tokenContractId: string,
  to: string,
  amount: bigint
): Promise<void> {
  await invoke(net, from, tokenContractId, "transfer", [
    new Address(from.publicKey()).toScVal(),
    new Address(to).toScVal(),
    nativeToScVal(amount, { type: "i128" }),
  ]);
}
