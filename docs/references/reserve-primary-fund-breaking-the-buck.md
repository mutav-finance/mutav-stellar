# Reserve Primary Fund — "Breaking the Buck" (Sep 2008)

- **Canonical URL:** SEC filings + post-mortem reporting (NYT / WSJ archives, "The Money Funds Reader")
- **Type:** TradFi precedent / case study / regulatory aftermath
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §3 (accounting model), §5 (NAV math), §7 (redemption)

## Summary

*[To be populated]* — The Reserve Primary Fund, a $62B money-market fund holding Lehman Brothers commercial paper, "broke the buck" on Sep 16, 2008 — NAV fell below $1.00 per share. Triggered a money-market run that nearly seized US short-term credit markets; only stopped by emergency Treasury guarantee. Aftermath: SEC Rule 2a-7 reforms, eventual variable-NAV (VNAV) requirements for institutional prime funds.

## Relevance to MGV

*[To be populated]* — **The seminal failure case for the constant-NAV model.** MGV uses appreciating-NAV (not constant-NAV), which structurally avoids the "breaking the buck" failure mode — there's no $1.00 peg to break. But the Reserve Primary episode is still relevant because:

- It demonstrates **run dynamics** when investors lose confidence in a yield-bearing pool
- It demonstrates why the "we have reserves, we're fine" message doesn't survive a stress event
- It motivates the redemption-cap / queue / pause design (MGV's `set_paused` is the emergency response analogous to what the Reserve Primary should have invoked earlier)
- It motivates the **NAV transparency** discipline — Reserve had been masking deterioration; on-chain NAV makes this impossible

Worth citing in §5 as the reason MGV chose appreciating-NAV over constant-NAV, and in §7 to justify the cap+queue architecture.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`money-market-constant-nav-history.md`](./money-market-constant-nav-history.md)
- [`uk-property-fund-suspensions.md`](./uk-property-fund-suspensions.md)
- [`franklin-onchain-fobxx.md`](./franklin-onchain-fobxx.md) — modern constant-NAV money-market fund on-chain
