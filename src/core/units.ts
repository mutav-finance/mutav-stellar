// Stellar Classic asset amounts are 7-decimal strings (e.g. "100.0000000").
// USDC inside the contract is 6-decimal bigint (e.g. 100_000_000n).
// This helper bridges the two when reading partner payments off Horizon.
export function parseStellarUsdc(amount: string): bigint {
  const [whole, frac = ""] = amount.split(".");
  return BigInt(whole + frac.padEnd(6, "0").slice(0, 6));
}
