# Soroban storage tiers

- **Canonical URL:** https://developers.stellar.org/docs/build/guides/storage
- **Type:** Spec / dev docs
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §11 (storage, TTL, resource bounds)

## Summary

*[To be populated]* — Three storage tiers in Soroban: `instance`, `persistent`, `temporary`. Each has distinct lifetime, cost, and archival semantics. Instance is single-cell-per-contract; persistent is per-key, archivable + restorable; temporary auto-deletes at TTL expiry and is not restorable.

## Relevance to MGV

*[To be populated]* — The contract's `DataKey` enum maps each state shape to one of the three tiers. The replay-guard's choice of `temporary` storage with a 7-day TTL is load-bearing: it bounds the contract's storage footprint while ensuring no Stellar tx can be replayed within its lifetime. The `Balance(Address)` choice of `persistent` is what allows investors to recover from archival via restoration.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`soroban-auth.md`](./soroban-auth.md) — auth-chain semantics
