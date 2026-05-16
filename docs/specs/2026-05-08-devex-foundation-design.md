# Devex foundation — design

**Date:** 2026-05-08
**Branch:** `chore/devex-foundation`
**Status:** Approved (brainstorm), pending implementation plan.

## Context

`mutav-stellar` is the Soroban smart-contracts repo for the SGR protocol, developed under the NearX acceleration program. The repo has a working `pre-push` hook, a `CLAUDE.md`, a `README.md`, and three uncommitted files dropped in earlier (`.editorconfig`, `.gitattributes`, `CONTRIBUTING.md`). The `CONTRIBUTING.md` references files that do not yet exist (`SECURITY.md`, structured issue templates), and no repo in the `mutav-finance` org has a `LICENSE`.

Before opening the repo to external eyes (auditors, integrators), it needs a coherent baseline.

## Goal

Make `mutav-stellar` legible to **future auditors and B2B integrators** (audience D) by:

- Adopting an OSI-approved license that integrators' legal teams accept without escalation.
- Providing a credible vulnerability-disclosure path.
- Fixing the dangling references in the in-flight `CONTRIBUTING.md` draft.
- Adding light formatting/linguist conventions so the repo reads cleanly on GitHub.

All of this must ship in **one bounded PR** off the existing `chore/devex-foundation` branch.

## Non-goals

- **No CI in this branch.** `mutav-stellar` has no `package.json` or contracts code yet; CI workflows would be broken on arrival. CI is deferred to the first Phase A PR.
- **No issue templates, no code of conduct.** Audience D (auditors, integrators) does not file GitHub issues; they email or use private security advisories.
- **No changes to sibling repos.** License mirroring across mutav, `mutav-solana`, `mutav-app` is tracked as separate follow-up issues; this PR does not silently drag them along.
- **No per-file SPDX license headers yet.** Top-level `LICENSE` + `NOTICE` is sufficient for Apache-2.0 compliance. Per-file headers land with Soroban contracts in Phase B.

## Decisions

### License: Apache-2.0

Selected over MIT, BUSL-1.1, dual Apache/MIT, and "defer."

**Rationale:**
- Stellar ecosystem norm — Stellar SDKs and Soroban examples are Apache-2.0. Anchor on Solana also Apache-2.0. Auditor and integrator legal teams approve it routinely.
- Explicit patent grant matters for a novel rental-guarantee primitive.
- BUSL-1.1 was rejected: integrator legal teams treat it as a yellow flag, and SGR is positioned as public-good infrastructure — there is no extractable-fee thesis that would justify the friction.
- "Defer" was rejected: integrator legal teams will not authorize work against an unlicensed repo, regardless of stated intent.

### Audience: D (auditors + B2B integrators)

Selected over internal-only, hackathon community, or general open-source contributors.

**Implications applied throughout the design:**
- Spec/whitepaper cross-links matter more than first-contributor friendliness.
- A clear non-public security channel matters more than a Code of Conduct.
- A reproducible-build story (when there is code to reproduce) matters more than issue templates.

### Security disclosure: GitHub private advisories only

No `security@` email until a real protocol address exists. GitHub's private vulnerability reporting is sufficient for current scale and is the modern default that auditors recognize.

### CONTRIBUTING pattern: delegate to canonical

The in-flight `CONTRIBUTING.md` already delegates to `mutav-finance/mutav/CONTRIBUTING.md` rather than duplicating it. This is the right pattern and should propagate protocol-wide (tracked as a follow-up issue against `mutav-solana`, which currently duplicates).

## File layout

```
mutav-stellar/
├── LICENSE                            NEW
├── NOTICE                             NEW
├── SECURITY.md                        NEW
├── CONTRIBUTING.md                    EDIT
├── README.md                          EDIT
├── .editorconfig                      NEW (already drafted, untracked)
├── .gitattributes                     NEW (drafted, trim brand/** line)
├── .github/
│   ├── CODEOWNERS                     NEW
│   └── pull_request_template.md       NEW
└── (existing: .githooks/, .gitignore, CLAUDE.md, .design/)
```

Eight new files, three edited. No `.github/workflows/`, no `.github/ISSUE_TEMPLATE/`.

## File contents

### `LICENSE`

Verbatim Apache-2.0 text from <https://www.apache.org/licenses/LICENSE-2.0.txt>.

### `NOTICE`

```
SGR Stellar
Copyright 2026 TGA Protocol contributors.

This product is part of SGR (Sistema de Garantia Registrada),
an onchain rental-guarantee protocol. See https://github.com/mutav-finance/mutav.
```

### `SECURITY.md`

Three sections, terse:

- **Reporting a vulnerability** — Use GitHub's private vulnerability reporting (Security tab → Report a vulnerability). Do not open a public issue.
- **Scope** — In scope: smart contracts (Phase B, future) and the TS API package (Phase A). Out of scope: third-party dependencies, Stellar core, dependent services.
- **Response expectations** — Acknowledge within 5 business days. Disclosure timeline coordinated with reporter. No fixed-day SLA.

### `CONTRIBUTING.md` edits

Apply to the in-flight draft:

1. Keep the link to `SECURITY.md` (file now exists).
2. Replace `"Bugs / features: open a GitHub issue using the structured templates."` → `"Bugs / features: open a GitHub issue. We don't use templates yet — a clear title with a reproducer is enough."`
3. Replace the entire License section body (`"License is currently **pending** for this repo. See the [README](./README.md) for status. By submitting a contribution, you agree your contribution will be licensed under whatever license is ultimately adopted..."`) with: `"Licensed under [Apache-2.0](./LICENSE). Contributions are accepted under the same license per Apache-2.0 §5."` The "ultimately adopted" disclaimer is stale once a license is in place.

### `README.md` edits

Append a License section:

```markdown
## License

Apache-2.0. See [LICENSE](./LICENSE) and [NOTICE](./NOTICE).
```

### `.editorconfig`

Drafted file; no changes from the version already in the working tree.

### `.gitattributes`

Drafted file; remove the line `brand/**       linguist-vendored` (no brand directory is vendored in this repo — mutav-solana has it, mutav-stellar does not).

### `.github/CODEOWNERS`

```
* @hoffms
```

Single maintainer; honest signal.

### `.github/pull_request_template.md`

```markdown
## What

<!-- What does this PR do? Keep it short. -->

## Why

<!-- Why is this change needed? Link issues if relevant. -->

## How to test

<!-- Steps to verify this works. -->
```

Adapted from `mutav-solana`'s template; `Phase` checklist (Solana-specific taxonomy) intentionally dropped.

## Commit strategy

Four atomic commits on the existing `chore/devex-foundation` branch, in order:

1. `chore(license): adopt Apache-2.0` — `LICENSE`, `NOTICE`, README license section, CONTRIBUTING "license pending" line.
2. `chore(security): add disclosure policy` — `SECURITY.md`, CONTRIBUTING SECURITY link unblocked.
3. `chore(devex): add editorconfig and gitattributes` — `.editorconfig`, `.gitattributes` (trimmed).
4. `chore(github): add CODEOWNERS and PR template` — `.github/CODEOWNERS`, `.github/pull_request_template.md`, CONTRIBUTING "structured templates" line softened.

Squash-merge to `main` per protocol convention. The squash commit message will summarize the foundation in Conventional-Commits form.

## Follow-up issues (out of scope)

To be opened after this PR merges:

- `mutav-finance/mutav` — Adopt Apache-2.0 (mirror mutav-stellar).
- `mutav-finance/mutav-solana` — Adopt Apache-2.0; convert duplicated `CONTRIBUTING.md` to delegate-to-canonical pattern.
- `mutav-finance/mutav-app` — Adopt Apache-2.0.
- `mutav-finance/mutav-stellar` — Wire CI (TS/Bun typecheck + test) on first Phase A PR.
- `mutav-finance/mutav-stellar` — Add per-file SPDX headers when Soroban contracts land (Phase B).

## Open assumptions

- **Copyright holder string** is `TGA Protocol contributors` in `NOTICE`. If the protocol entity adopts a formal legal name (e.g., a foundation), `NOTICE` will need a one-line update.
- **Security disclosure email** is intentionally omitted; if a `security@` address is provisioned later, append it to `SECURITY.md`.
- **CODEOWNERS** lists only `@hoffms`. If additional maintainers join before Phase B, expand.
