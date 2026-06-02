# Soroban authorization model

- **Canonical URL:** https://developers.stellar.org/docs/learn/encyclopedia/security/authorization
- **Type:** Spec / dev docs
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §10 (authority model), §6–§7 (USDC interactions)

## Summary

*[To be populated]* — Soroban's authorization is opt-in via `Address::require_auth()`. Each `require_auth()` is satisfied either by a direct signature from the address (in the outer transaction's auth payload) or by a chained authorization granted by a contract that itself was authorized. Sub-invocations of contracts can require their own auth, forming a tree.

## Relevance to MGV

*[To be populated]* — `require_operator()`, `require_admin()`, `investor.require_auth()` patterns are all direct `require_auth` calls. The chained-auth behavior is what allows `usdc.transfer(operator, contract, amount)` inside `receive_payment` to succeed when only the operator signed the outer transaction — Soroban computes the required sub-auth tree during simulation and the operator's signature covers it.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`sep-41.md`](./sep-41.md) — SEP-41 token auth semantics
