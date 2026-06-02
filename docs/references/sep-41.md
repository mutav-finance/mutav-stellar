# SEP-41 — Stellar Token Interface

- **Canonical URL:** https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md
- **Type:** Ecosystem standard
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §4 (share token), §6–§7 (USDC interactions)

## Summary

*[To be populated]* — Standard token interface for Soroban contracts; the analogue of ERC-20 for Stellar. Defines `transfer`, `transfer_from`, `approve`, `balance`, `name`, `symbol`, `decimals`, `burn`, `burn_from`.

## Relevance to MGV

*[To be populated]* — The MUTAV share token must implement SEP-41 for DeFi composability. USDC (and any other underlying asset) is consumed via the same SEP-41 interface through `token::Client` from `soroban-sdk`. Auth-chain semantics for `transfer_from` matter for the operator-mediated USDC moves.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`soroban-auth.md`](./soroban-auth.md) — auth-chain semantics
