# Ensuro — RWA-Bridged Insurance Capital

- **Canonical URL:** https://ensuro.co/ and https://docs.ensuro.co/
- **Type:** Production system / RWA insurance bridge
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §9 (default coverage), §13 (open decisions)

## Summary

*[To be populated]* — Ensuro is an RWA insurance protocol that bridges DeFi capital to traditional insurance underwriting. Insurance carriers (off-chain regulated entities) issue policies; Ensuro pools provide the underlying capital via DeFi LPs; smart contracts handle premium collection, claim settlement, and yield distribution. Effectively: **DeFi capital backs off-chain insurance product**.

## Relevance to MGV

*[To be populated]* — **Most structurally similar to MGV's actual situation.** MGV's product is a regulated off-chain rental guarantee (issued by the BR-regulated `Mutav Soluções` entity); MGV's vault is the DeFi capital pool that backs the guarantee. Ensuro has solved several of the exact same problems MGV faces:

- How does on-chain capital back an off-chain regulated product?
- How does the regulated entity's claim decision propagate on-chain?
- How do you handle the actuarial / risk-pricing layer when the underwriting happens off-chain?
- How does redemption work when capital is committed to active policies of multi-month duration?

The Ensuro-style "bridge" architecture might be the right framing for MGV. Worth a dedicated study during the research pass, including:
- Their capital-lock / unlock-on-policy-expiry mechanics (compare to MGV's exit cap)
- Their multi-pool / multi-policy architecture
- How they bridge off-chain claim decisions on-chain

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`nexus-mutual.md`](./nexus-mutual.md) — discretionary on-chain insurance counterpart
- [`etherisc.md`](./etherisc.md) — parametric counterpart
- [`maple-finance.md`](./maple-finance.md) — adjacent capital-backs-off-chain-obligation model
