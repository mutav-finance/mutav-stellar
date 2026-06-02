# BlackRock BUIDL — Institutional Digital Liquidity Fund

- **Canonical URL:** https://securitize.io/learn/press/blackrock-launches-first-tokenized-fund-buidl-on-the-ethereum-network
- **Type:** Production system / tokenized RWA fund
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §3 (accounting model), §6 (inflow), §13 (open decisions)

## Summary

*[To be populated]* — BlackRock USD Institutional Digital Liquidity Fund (BUIDL), launched March 2024 on Ethereum (later multi-chain via Securitize). First tier-1 asset manager to issue a tokenized fund. Backed by US T-Bills, cash, and repo. Pays daily-accrued yield as additional tokens to holders (rebase / dividend distribution rather than NAV appreciation). ERC-20 token, transferable only to whitelisted addresses.

## Relevance to MGV

*[To be populated]* — Closest tier-1 precedent for a tokenized fund with off-chain reserve + on-chain share token + operator-mediated yield posting. Key differences worth surfacing:

- BUIDL: ERC-20 dividend-distribution model (mint new tokens for accrued yield) → MGV: NAV-appreciation model (yield reflects in share price). Two valid paradigms; comparing them is load-bearing for §5.
- BUIDL: whitelist-enforced transferability via Securitize compliance → MGV: full SEP-41, KYC enforced off-chain on `mutav-app`. Choice has audit-surface + composability implications.
- BUIDL: institutional-only ($5M minimum) → MGV: retail-accessible. Affects share-inflation defense thresholds.
- BUIDL: synchronous redemption against Circle USDC liquidity → MGV: async queue. BUIDL has a backstop liquidity facility; MGV has the weekly exit cap.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`ondo-finance.md`](./ondo-finance.md) — direct EVM competitor
- [`franklin-onchain-fobxx.md`](./franklin-onchain-fobxx.md) — first money-market fund on-chain
- [`erc-4626.md`](./erc-4626.md) — BUIDL predates 4626 but uses similar share semantics
