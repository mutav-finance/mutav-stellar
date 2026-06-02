# Maple Finance — Tokenized Credit Pools

- **Canonical URL:** https://maple.finance/ and https://maplefinance.gitbook.io/maple/
- **Type:** Production system / on-chain credit
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §3 (accounting model), §7 (redemption), §9 (default coverage), §13 (open decisions)

## Summary

*[To be populated]* — Maple Finance is an on-chain credit protocol: pool delegates underwrite loans to institutional borrowers; LPs deposit stablecoins into pools and receive pool-token shares; loans pay back interest, defaults are absorbed by pool LPs in proportion to their share. Multiple defaults (Orthogonal Trading, M11 Credit pools 2022–2023) provide concrete failure-mode references.

## Relevance to MGV

*[To be populated]* — Maple is **the most direct precedent for MGV's actual risk profile**: LPs absorb credit losses on a pool that underwrites third-party obligations. Differences:

- Maple: undercollateralized loans → MGV: rental guarantee (also undercollateralized; defaults paid from reserve)
- Maple: pro-rata absorption of losses, no waterfall → MGV: single-tranche today (per scope decision)
- Maple: long withdrawal cycles (30-day notice for some pools, 90-day for others) → MGV: weekly cap + queue
- Maple: had to invent emergency redemption gates after Orthogonal default → MGV's `set_paused` is the equivalent

The Orthogonal Trading default + Maple's response (loss socialization mechanics, gate activation, fund-restart workflow) is **highly instructive** for §9 (default coverage) and §12 (adversarial). Worth a dedicated case-study sub-section.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`nexus-mutual.md`](./nexus-mutual.md) — adjacent mutualization concept
- [`centrifuge.md`](./centrifuge.md) — tranched credit alternative
- [`blackstone-breit-gating.md`](./blackstone-breit-gating.md) — TradFi gating precedent
