# Memory — mutav-stellar

Durable facts that survive context resets. Append as decisions land; remove when superseded.

## Repo identity

- **Audited surface.** Soroban `Fund` contract (Rust) + read-oriented TS SDK published as `@mutav-finance/mutav-stellar`.
- **No keys here.** Operator authority lives on `mutav-app` (KMS-backed Convex Actions); admin authority is a hardware wallet inside `mutav-app/apps/admin/`. The SDK composes XDRs; consumers sign.
- **NearX acceleration cohort** (program participation; affects cadence + review gates).

## Repo split (2026-05-30)

Consolidated from three repos to two. The 6 in-flight operator daemons (PRs #22–#27) are out of scope here — they move to `mutav-app` as KMS-backed Convex Actions. See [`docs/architecture/decisions/2026-05-30-daemon-prs-orphan-verdict.md`](docs/architecture/decisions/2026-05-30-daemon-prs-orphan-verdict.md) for the orphan verdict.

## Architecture decisions

ADRs live at [`docs/architecture/decisions/`](docs/architecture/decisions/). Read these before reopening a decided question. Add a new ADR whenever an audit finding, PR comment, or design discussion produces a load-bearing decision.

## Open questions / pending decisions

(none recorded yet — add `## YYYY-MM-DD — <decision>` blocks as they land)
