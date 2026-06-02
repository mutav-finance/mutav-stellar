# Onre Finance — Solana-Native RWA Tokenization

- **Canonical URL:** https://onre.io/
- **Type:** Production system / tokenized RWA on Solana
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §3 (accounting model), §13 (multi-chain considerations)

## Summary

*[To be populated]* — Onre Finance is a Solana-native protocol for tokenizing yield-bearing real-world assets, with a focus on retail-accessible RWA exposure. Operates on the SPL token standard, leveraging Solana's high throughput + low fees for retail-scale issuance / redemption flows.

## Relevance to MGV

*[To be populated]* — **Solana-side reference** for the multi-chain story. MUTAV has a parallel `mutav-solana` Anchor implementation (Colosseum Frontier hackathon). Onre is the closest production precedent on Solana for what MGV is doing on Stellar:

- Solana-native vault primitives (Anchor program patterns)
- SPL token vs SEP-41 — what changes between the two share-token implementations
- Solana's account model implications for vault state (vs Soroban's contract storage)
- Retail-flow throughput characteristics — what scale do these protocols actually run at on Solana
- Latin American positioning (if applicable — verify during research pass)

Useful as a "we considered this, here's how it compares" reference rather than a direct design precedent.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`ondo-finance.md`](./ondo-finance.md) — multi-chain EVM-side counterpart
- [`blackrock-buidl.md`](./blackrock-buidl.md)
- [`franklin-onchain-fobxx.md`](./franklin-onchain-fobxx.md)
