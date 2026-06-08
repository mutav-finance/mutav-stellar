# Security Policy

## Reporting a vulnerability

Use GitHub's private vulnerability reporting (Security tab → **Report a vulnerability**). Do **not** open a public issue for security reports.

If GitHub private reporting is unavailable for any reason, contact the maintainer directly via the email listed on their GitHub profile.

## Scope

In scope:

- Smart contracts (Phase B, future).
- The TypeScript API package (Phase A, `@mutav-finance/mutav-stellar`).

Out of scope:

- Third-party dependencies — please report upstream.
- Stellar core / network infrastructure — report to the Stellar Development Foundation.
- Services or applications that depend on this repository.

## Response expectations

We will acknowledge your report within **5 business days**. Disclosure timing will be coordinated with the reporter; we do not commit to a fixed-day SLA at this stage.

## Response procedure

After a vulnerability report is accepted for triage, maintainers follow the
[vulnerability disclosure incident response runbook](docs/security/incident-response.md)
for severity assignment, containment, patch staging, cross-repo coordination,
disclosure timing, and post-incident follow-up.
