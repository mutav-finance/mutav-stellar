# Ondo Finance — OUSG / USDY

- **Canonical URL:** https://docs.ondo.finance/
- **Type:** Production system / tokenized RWA fund
- **First read:** *TBD*
- **Cited from:** MGV whitepaper §3 (accounting model), §5 (NAV math), §6 (inflow), §7 (redemption)

## Summary

*[To be populated]* — Ondo Finance offers two flagship tokenized products:

- **OUSG** (Ondo Short-Term US Government Bond Fund) — institutional-only, KYC-gated, BlackRock-backed (originally; now diversified). Token represents pro-rata claim on a short-duration treasury fund. Redemption via Ondo's compliance flow.
- **USDY** (Ondo US Dollar Yield) — retail-accessible (non-US persons), permissionlessly transferable on multiple chains. NAV-appreciating token backed by short-term US treasuries + bank deposits. Daily NAV update by Ondo operations.

## Relevance to MGV

*[To be populated]* — USDY is the closest design precedent to MGV: NAV-appreciating share token, off-chain reserve, operator-mediated NAV updates, async redemption (T+1 / T+2). What's worth pulling:

- NAV update cadence + on-chain transparency (Ondo publishes daily NAV via attestation)
- Redemption window mechanics under stress (Ondo experienced redemption flow stress in Mar 2023 bank-crisis weekend)
- USDY's choice to keep the token permissionless-transferable (composability) while restricting *issuance* to KYC — exactly the model MGV is converging on
- Reserve-attestation reporting requirements (monthly + on-chain proofs)

OUSG is more relevant as a regulatory-tier reference (institutional, gated); USDY is more relevant mechanically.

## Annotated takeaways

*[To be populated]*

## Notable quotations

*[To be populated]*

## Cross-references

- [`blackrock-buidl.md`](./blackrock-buidl.md)
- [`franklin-onchain-fobxx.md`](./franklin-onchain-fobxx.md)
- [`erc-4626.md`](./erc-4626.md)
