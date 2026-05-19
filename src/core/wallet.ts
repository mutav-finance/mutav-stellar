import { Keypair } from "@stellar/stellar-sdk";

export function loadOperatorKeypair(): Keypair {
  const secret = process.env.OPERATOR_SECRET;
  if (!secret) throw new Error("OPERATOR_SECRET não definido no .env.local");
  return Keypair.fromSecret(secret);
}

export function loadFundContractId(): string {
  const id = process.env.FUND_CONTRACT_ID;
  if (!id) throw new Error("FUND_CONTRACT_ID não definido no .env.local");
  return id;
}
