# OpenZeppelin — ERC-4626 Inflation-Attack Mitigation (virtual shares)

- **Canonical URL:** https://docs.openzeppelin.com/contracts/4.x/erc4626 (current version)
- **Type:** Spec extension / mitigation analysis
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §5 (NAV math), §12 (adversarial properties)

## Summary

*[To be populated]* — OpenZeppelin's ERC-4626 implementation introduces "virtual shares" + "virtual assets" — a constant offset added to both `totalSupply` and `totalAssets` during conversion math. The offset asymptotically eliminates the share-inflation attack surface: an attacker would need to donate `O(10^N)` underlying to manipulate the price by `1/N` of a share.

## Relevance to MGV

*[To be populated]* — Direct mitigation technique for the share-inflation class of bugs (see [`cream-finance-inflation-incident.md`](./cream-finance-inflation-incident.md)). MGV currently does NOT implement virtual shares. Whether to add it depends on:

- The deployment model: if MGV vaults are only deployed by admin (not user-permissionless), the attacker class is much smaller
- The first-deposit semantics: MGV's `NAV₀ = 1.0` handles the divide-by-zero case but doesn't bound first-attacker advantage
- Cost-benefit: virtual shares add 1 extra read/write per conversion and shift rounding outcomes by 1 share unit

Decision point for §13: should MGV adopt virtual shares?

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`erc-4626.md`](./erc-4626.md)
- [`cream-finance-inflation-incident.md`](./cream-finance-inflation-incident.md)
- [`openzeppelin-stellar-contracts-vault.md`](./openzeppelin-stellar-contracts-vault.md) — should also adopt this when complete
