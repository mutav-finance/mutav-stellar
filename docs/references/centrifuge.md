# Centrifuge — Tokenized RWA with Tranches

- **Canonical URL:** https://docs.centrifuge.io/
- **Type:** Production system / tokenized RWA with structured credit
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §13 (open decisions — tranche composition as GTM)

## Summary

*[To be populated]* — Centrifuge tokenizes real-world receivables (invoices, real estate loans, consumer credit, BlockTower fund, etc.) into on-chain pools with **junior / senior tranche structures**. Senior tranche holders earn lower-but-stable yield; junior tranche absorbs first-loss in exchange for higher yield. Uses Tinlake (Ethereum) + Centrifuge Chain (Substrate).

## Relevance to MGV

*[To be populated]* — Centrifuge is the **on-chain tranche-and-waterfall precedent**. MGV's current scope decision (per user direction) is: tranches are a GTM concern (multi-vault deployment with off-chain orchestration), not a vault primitive concern. Centrifuge demonstrates the alternative — implementing tranches *inside* the protocol. Worth surfacing in §13's open-decisions discussion:

- Centrifuge tranche absorption is fully on-chain
- It requires multi-token state (junior + senior tokens) inside the protocol
- Audit surface is materially larger as a result
- Argument for / against doing this at the protocol layer

Also useful: Centrifuge's experience with tranche-waterfall under stress (default events in the BlockTower pool, GIG pool).

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`maple-finance.md`](./maple-finance.md) — non-tranched credit pool counterpart
- [`enzyme-finance.md`](./enzyme-finance.md)
- [`morpho-blue.md`](./morpho-blue.md) — minimal-primitive argument
