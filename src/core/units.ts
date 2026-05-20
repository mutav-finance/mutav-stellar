// Stellar represents asset amounts with 7 decimal places (e.g. "100.0000000").
// USDC in the contract uses 6 decimals (e.g. 100_000_000n).
export function parseStellarUsdc(amount: string): bigint {
  const [whole, frac = ""] = amount.split(".");
  return BigInt(whole + frac.padEnd(6, "0").slice(0, 6));
}
