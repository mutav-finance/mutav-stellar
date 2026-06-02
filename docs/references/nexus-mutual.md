# Nexus Mutual — On-Chain Mutual Insurance

- **Canonical URL:** https://docs.nexusmutual.io/
- **Type:** Production system / on-chain mutual insurance
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §9 (default coverage), §12 (adversarial properties), §13 (open decisions)

## Summary

*[To be populated]* — Nexus Mutual is a discretionary on-chain mutual insurance protocol. Members pool capital (in ETH / DAI / NXM token); they vote on claim payouts via NXM-token-weighted governance. Originally covered smart-contract failure risk, expanded to custodian failure, yield protocol risk, slashing risk, etc. The mutual structure means coverage capacity is bounded by the pooled capital — analogous to MGV's AUM being the guarantee backing.

## Relevance to MGV

*[To be populated]* — **The closest structural precedent to MGV's product.** v0 whitepaper claims reinsurance as the analogue; Nexus Mutual is the on-chain incarnation of that. Key parallels:

- Members pool capital → MGV: investors pool USDC into the vault
- Capital backs coverage commitments → MGV: AUM backs rental guarantees
- Claims paid from pool → MGV: `cover_default` debits AUM
- Coverage capacity is bounded by pooled capital → MGV: same; the vault can't underwrite more guarantee value than its AUM
- Member shares appreciate / depreciate based on claims experience → MGV: NAV moves with `add_yield` (premium income) and `cover_default` (claims)

Differences:
- Nexus: claim assessment is **discretionary** (member-voted) → MGV: claim assessment is **operator-discretionary** (admin calls `cover_default`)
- Nexus: insurance pricing via risk-assessment vote → MGV: fee per contract determined by off-chain `mutav-app` risk-scoring
- Nexus: open-membership → MGV: closed (operator + admin curate everything)

Worth pulling: Nexus's capital-vs-coverage-ratio modeling, claim-incident response process, NXM bonding curve (probably not directly applicable but instructive).

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`etherisc.md`](./etherisc.md) — parametric (non-discretionary) counterpart
- [`ensuro.md`](./ensuro.md) — RWA-bridged version
- [`maple-finance.md`](./maple-finance.md) — credit-loss-absorption parallel
