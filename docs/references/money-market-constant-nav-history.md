# Money-Market Funds — Constant-NAV vs Variable-NAV History

- **Canonical URL:** SEC Rule 2a-7 history; ICI (Investment Company Institute) research papers; Federal Reserve papers on MMF reforms (2010, 2014, 2023)
- **Type:** Regulatory history / academic reference
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §3 (accounting model), §5 (NAV math), §13 (open decisions)

## Summary

*[To be populated]* — Money-market funds historically used **constant-NAV** ($1.00 share price; yield distributed as dividends), allowing them to be marketed as cash-equivalents. The 2008 Reserve Primary "breaking the buck" event triggered three rounds of SEC reform (2010, 2014, 2023) that progressively forced **variable-NAV** (VNAV) for institutional prime funds, retained constant-NAV for retail government MMFs, and added liquidity-fee + redemption-gate tools. The history is the canonical case study for why share-price design matters.

## Relevance to MGV

*[To be populated]* — **The most-cited academic reference for the constant-NAV vs appreciating-NAV design choice.** MGV chose appreciating-NAV; this reference grounds *why*:

- Constant-NAV requires the fund to maintain a $1.00 peg → MGV would need a peg-defense mechanism, which is fragile
- Constant-NAV creates "breaking the buck" as a discrete failure mode → MGV's appreciating-NAV has no analogous cliff; depreciation is continuous and observable
- Constant-NAV typically requires asset-class restrictions (Rule 2a-7) to maintain → MGV's underlying (TESOURO + receivables) wouldn't qualify under 2a-7 anyway
- The 2014 SEC reforms introduced **liquidity fees + redemption gates** for prime MMFs — exactly MGV's design (redemption fee + weekly cap)

So MGV's design is structurally aligned with **post-2014 SEC reform thinking on MMF resilience**. Worth surfacing this explicitly.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`reserve-primary-fund-breaking-the-buck.md`](./reserve-primary-fund-breaking-the-buck.md)
- [`franklin-onchain-fobxx.md`](./franklin-onchain-fobxx.md) — modern constant-NAV MMF on-chain
- [`cfa-alt-investments-reading.md`](./cfa-alt-investments-reading.md)
