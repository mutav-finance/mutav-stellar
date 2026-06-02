# CFA Institute — Alternative Investments (NAV / Fee Mechanics)

- **Canonical URL:** CFA Program Curriculum, L2 / L3 Alternative Investments readings (CFA Institute, current edition)
- **Type:** Educational / canonical reference
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §3 (accounting model), §5 (NAV math), §8 (fees)

## Summary

*[To be populated]* — The CFA Program's Alternative Investments readings cover the canonical fund-accounting concepts: striking NAV (when, frequency, fair value vs market vs amortized cost), management fees (asset-based vs performance-based), high-water marks, hurdle rates, side pockets, carry / waterfall structures, and the accounting treatments that bridge TradFi fund design to retail-readable disclosures.

## Relevance to MGV

*[To be populated]* — Canonical reference for fund-accounting conventions MGV inherits or rejects:

- **NAV striking frequency** — MGV computes NAV on every state-changing call (effectively continuous); TradFi convention is daily-strike for liquid funds, monthly/quarterly for illiquid. The "per event" choice in MGV is unusual; worth grounding the choice.
- **Fair-value rules** — MGV's AUM is operator-reported (no on-chain mark-to-market); CFA's framework for level-1/2/3 valuation explains why this is acceptable for off-chain reserves but exposes residual trust.
- **Management fee mechanics** — CFA convention is mgmt fee accrued continuously and crystalized periodically (monthly). MGV's 30-day interval matches; worth citing.
- **Performance fee / high-water mark** — MGV does NOT charge performance fees. Worth explaining why (operator can't manipulate when yield isn't endogenous).
- **Side pockets** — what they are, why MGV doesn't have them, what would trigger needing them.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`aima-hedge-fund-accounting.md`](./aima-hedge-fund-accounting.md)
- [`money-market-constant-nav-history.md`](./money-market-constant-nav-history.md)
- [`enzyme-finance.md`](./enzyme-finance.md)
