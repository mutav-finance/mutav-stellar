# Yearn V2 — Vault Architecture

- **Canonical URL:** https://docs.yearn.fi/developers/v2/specification
- **Type:** Production system / docs
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §5 (NAV math), §6 (inflow), §7 (redemption)

## Summary

*[To be populated]* — Yearn V2 vaults wrap a single underlying token (e.g. DAI) and route capital to one or more strategies. Share-token semantics analogous to ERC-4626 (which Yearn predates and influenced).

## Relevance to MGV

*[To be populated]* — Yearn's strategy router and "credit / debt" accounting model is overkill for MGV (which has a single off-chain strategy: TESOURO via Etherfuse). What's worth pulling:

- The `price_per_share` accessor pattern (analogous to MGV's `nav()`)
- Deposit / withdrawal locking around `harvest` events (analogous to MGV's pre-fund step before `fulfill_redemption`)
- Yearn's experience with share-price manipulation via direct token transfers

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`erc-4626.md`](./erc-4626.md)
- [`enzyme-finance.md`](./enzyme-finance.md)
