// XDR operation builders for the MUTAV `Fund` Soroban contract.
//
// These are pure functions of (contractId, args) — no network access, no signing.
// Consumers (typically a Convex Action with KMS-backed signing on `mutav-app`)
// assemble the returned `xdr.Operation` into a TransactionBuilder, prepare
// against an RPC server, sign with their KMS-sourced key, and submit.
//
// Scope per `mutav-stellar#57`: the SDK composes chain reads + transaction
// XDRs; it does not hold keys and does not submit. The operator-runtime
// signing pathway lives on mutav-app.

import { Address, Contract, nativeToScVal, xdr } from "@stellar/stellar-sdk";

function fund(contractId: string): Contract {
  return new Contract(contractId);
}

function i128(amount: bigint): xdr.ScVal {
  return nativeToScVal(amount, { type: "i128" });
}

function bytes32(value: Uint8Array): xdr.ScVal {
  if (value.length !== 32) {
    throw new Error(`bytes32 expected 32 bytes, got ${value.length}`);
  }
  return xdr.ScVal.scvBytes(Buffer.from(value));
}

export function buildReceivePaymentOp(
  contractId: string,
  imobiliaria: string,
  amountUsdc: bigint,
  txHash: Uint8Array,
): xdr.Operation {
  return fund(contractId).call(
    "receive_payment",
    new Address(imobiliaria).toScVal(),
    i128(amountUsdc),
    bytes32(txHash),
  );
}

export function buildAddYieldOp(contractId: string, amountUsdc: bigint): xdr.Operation {
  return fund(contractId).call("add_yield", i128(amountUsdc));
}

export function buildChargeMgmtFeeOp(contractId: string): xdr.Operation {
  return fund(contractId).call("charge_mgmt_fee");
}

export function buildProcessRedemptionsOp(contractId: string): xdr.Operation {
  return fund(contractId).call("process_redemptions");
}

export function buildFulfillRedemptionOp(
  contractId: string,
  investor: string,
): xdr.Operation {
  return fund(contractId).call("fulfill_redemption", new Address(investor).toScVal());
}

export function buildRecordOffchainPayoutOp(
  contractId: string,
  amountUsdc: bigint,
  destination: string,
): xdr.Operation {
  return fund(contractId).call(
    "record_offchain_payout",
    i128(amountUsdc),
    new Address(destination).toScVal(),
  );
}

export function buildExtendTtlOp(contractId: string): xdr.Operation {
  return fund(contractId).call("extend_ttl");
}

export function buildExtendBalanceTtlOp(
  contractId: string,
  investor: string,
): xdr.Operation {
  return fund(contractId).call("extend_balance_ttl", new Address(investor).toScVal());
}
