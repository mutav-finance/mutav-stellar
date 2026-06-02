# Enzyme Finance — Asset Management Protocol

- **Canonical URL:** https://docs.enzyme.finance/
- **Type:** Production system / docs
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §3 (accounting model), §4 (share token), §6 (inflow), §8 (fees), §10 (authority model)

## Summary

*[To be populated]* — Enzyme (formerly Melon Protocol) is a decentralized asset-management protocol on Ethereum. Vaults hold a basket of supported assets; managers (or smart contract strategies) trade those assets; investors hold vault shares that track NAV. Architecturally composed of: Vault (holds assets + share token), Comptroller (mediates state changes), Fee Manager (mgmt + perf fees), Policy Manager (deposit allow-list, deposit cap, etc.).

## Relevance to MGV

*[To be populated]* — Enzyme is the most direct precedent for an on-chain vault with **operator-mediated state changes**, **policy abstractions** (analogous to MGV's `set_approved_partner` + paused state), **fee accounting** (analogous to `charge_mgmt_fee`), and **share-token semantics**. Differences worth noting:

- Enzyme is multi-asset → MGV is single-asset (USDC). MGV's accounting is consequently much simpler.
- Enzyme's "Comptroller" mediates Vault state changes via authorized calls — MGV folds that role into the Fund contract itself with `require_operator` / `require_admin` checks.
- Enzyme supports a "Performance Fee" — MGV explicitly does not (yield is realized off-chain in TESOURO + reflected in `add_yield`, no high-water-mark concept).
- Enzyme's redemption is synchronous (pro-rata against current holdings); MGV's is async (queue + weekly cap) because the underlying is non-liquid TESOURO.

Worth pulling: fee snapshot semantics, deposit-cap pattern, asset-whitelist policy.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`erc-4626.md`](./erc-4626.md) — Enzyme predates ERC-4626 but shares the share-token paradigm
- [`yearn-v2.md`](./yearn-v2.md) — Yearn's strategy-router model is the EVM peer
