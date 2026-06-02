# Etherisc — Parametric Insurance Protocol

- **Canonical URL:** https://etherisc.com/ and https://docs.etherisc.com/
- **Type:** Production system / parametric on-chain insurance
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §9 (default coverage), §12 (adversarial properties)

## Summary

*[To be populated]* — Etherisc is a decentralized insurance protocol focused on **parametric** products — payouts triggered by objective, data-feed-verifiable conditions (flight delay, crop weather, hurricane intensity) rather than discretionary claim assessment. Generic Insurance Framework (GIF) provides the smart-contract layer; specific products (FlightDelay, Crop Insurance) sit on top.

## Relevance to MGV

*[To be populated]* — Less directly applicable than Nexus Mutual (MGV's claim trigger is *not* parametric — it requires off-chain default verification), but the **architectural separation** is highly relevant: Etherisc cleanly separates the "vault that holds reserves" from the "product that defines when to pay out". This is exactly the payment-system-split direction MGV is moving (see §13a).

Key patterns worth pulling:
- Product / risk-pool / vault separation
- Oracle integration patterns (data feed → payout trigger)
- Pool depletion / replenishment mechanics
- The argument for / against parametric vs discretionary claim assessment

If MGV's `cover_default` ever becomes oracle-triggered (e.g., on-chain attestation of unpaid rent), Etherisc's patterns become directly applicable.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`nexus-mutual.md`](./nexus-mutual.md) — discretionary counterpart
- [`ensuro.md`](./ensuro.md) — RWA-bridge insurance
