# Cream Finance — Share-Inflation Incident (Oct 2021)

- **Canonical URL:** https://medium.com/cream-finance/post-mortem-exploit-oct-27-507b12bb6f8e (primary post-mortem)
- **Type:** Incident report / failure analysis
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §5 (NAV math — share inflation), §12 (adversarial properties)

## Summary

*[To be populated]* — Cream Finance (an Iron Bank fork of Compound v2) suffered a $130M exploit where an attacker manipulated the share-price (cToken `exchangeRate`) via direct underlying-token donations to inflate their position before redemption. The exploit was not a contract bug per se — it was an emergent vulnerability of the share-price model when combined with insufficient initial liquidity in a market.

## Relevance to MGV

*[To be populated]* — Cream is the most-cited real-world share-inflation incident. The mechanics: a small-supply pool + an attacker who deposits 1 unit + directly transfers a large amount → share price spikes → attacker redeems for the inflated price. MGV's defenses:

1. `add_yield`'s `max_aum_increase_bps` per-call cap blocks the "direct donation" path *if the donation goes through `add_yield`*
2. **But** direct USDC transfers to the contract address don't go through `add_yield` — they sit as a balance the contract doesn't account for, until `sweep_usdc` moves them to `classic_wallet` (without crediting AUM)
3. So MGV is structurally resistant: AUM is only credited via authorized operator paths
4. Edge case: the off-contract pre-fund step (§7) temporarily lands USDC in the contract that isn't accounted in AUM. The window between pre-fund and `fulfill_redemption` could theoretically be exploited — analyze whether `request_redemption` during that window can mint inflated shares.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`erc-4626.md`](./erc-4626.md) — virtual-shares mitigation was a direct response to incidents like this
- [`compound-ctoken.md`](./compound-ctoken.md) — Cream forked Compound; same `exchangeRate` mechanism
- [`openzeppelin-virtual-shares.md`](./openzeppelin-virtual-shares.md)
