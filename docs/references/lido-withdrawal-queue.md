# Lido — Withdrawal Queue (async redemption)

- **Canonical URL:** https://docs.lido.fi/contracts/withdrawal-queue-erc721
- **Type:** Production system / docs
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §7 (redemption mechanism)

## Summary

*[To be populated]* — Lido's stETH withdrawal flow is the canonical on-chain async-redemption queue. Investors submit a withdrawal request, receive an NFT representing their queue position, and the request is processed in FIFO order as ETH is unstaked from validators. The mechanism is required because validator exits are non-instant.

## Relevance to MGV

*[To be populated]* — Lido and MGV face the same structural problem: the underlying asset is non-liquid on demand. MGV's TESOURO conversion via Etherfuse is the analog of Lido's validator-exit delay. The redemption queue design is conceptually identical (FIFO, deadline, partial fulfillment). Worth pulling:

- NFT-vs-mapping for queue-position representation — Lido uses ERC-721 for transferable claims; MGV uses an `Address → ReadyRedemption` mapping (non-transferable).
- Batch-fulfillment math (Lido's `findCheckpointHints`)
- The `claim` step semantics (investor pulls, vs. operator pushes)

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`enzyme-finance.md`](./enzyme-finance.md) — synchronous redemption counterpart
- [`erc-4626.md`](./erc-4626.md)
