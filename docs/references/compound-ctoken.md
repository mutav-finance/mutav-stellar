# Compound — cToken Exchange-Rate Model

- **Canonical URL:** https://docs.compound.finance/v2/ctokens/
- **Type:** Production system / docs
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §3 (accounting model), §5 (NAV math)

## Summary

*[To be populated]* — Compound v2's cTokens represent shares of a lending pool with an appreciating exchange rate (`exchangeRate = (totalCash + totalBorrows − totalReserves) / totalSupply`). The cToken model predates ERC-4626 and demonstrates the appreciating-share-price paradigm at scale.

## Relevance to MGV

*[To be populated]* — MGV's `NAV = aum / supply` is structurally the same as `cToken.exchangeRate`. Differences:

- Compound's `aum` (total cash + borrows − reserves) is on-chain trackable from the lending pool's own state; MGV's `aum` is operator-reported because the underlying (TESOURO) lives off-chain.
- Compound has no redemption queue — redemptions are pro-rata against on-chain cash, with a `redeemUnderlying` failure if cash is exhausted.
- Both models suffer from share-inflation if the first depositor is small and direct transfers occur. Compound's mitigation: minimum initial liquidity bootstrap.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`erc-4626.md`](./erc-4626.md)
- [`cream-finance-inflation-incident.md`](./cream-finance-inflation-incident.md) — Cream's exchange-rate exploitation; Cream forked Compound
