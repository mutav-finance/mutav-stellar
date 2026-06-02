# Blackstone BREIT — Redemption Gating Events (2022–2023)

- **Canonical URL:** https://www.breit.com/ + reporting (Reuters / WSJ / FT)
- **Type:** TradFi precedent / case study
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §7 (redemption mechanism), §13 (open decisions)

## Summary

*[To be populated]* — Blackstone Real Estate Income Trust (BREIT) — a $69B non-traded REIT — hit its 2% monthly / 5% quarterly redemption caps in late 2022 and proportionally gated investor withdrawals for ~14 consecutive months (Nov 2022 → Feb 2024). Redemption requests exceeded available liquidity; the gate worked as designed but caused significant investor friction and reputational impact.

## Relevance to MGV

*[To be populated]* — **The most cited modern TradFi precedent for exit-cap-based redemption gating under stress.** MGV's `exit_cap_bps` (default 10% per week) is structurally analogous to BREIT's 2% monthly / 5% quarterly. The BREIT episode answers questions MGV will face:

- What happens to user trust when the gate is hit for months?
- How does the operator communicate during a sustained gating event?
- Does the gate prevent fund collapse (yes, BREIT survived) or just delay it?
- What happens to inflows during a gating event? (BREIT saw inflows continue; instructive)
- What's the "honest" exit cap level — too tight = poor UX in good times; too loose = no protection in bad times?

Worth surfacing the BREIT analysis as a sanity check on MGV's 10%/week default cap.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`uk-property-fund-suspensions.md`](./uk-property-fund-suspensions.md) — broader gating precedent
- [`maple-finance.md`](./maple-finance.md) — on-chain gating precedent
- [`lido-withdrawal-queue.md`](./lido-withdrawal-queue.md)
