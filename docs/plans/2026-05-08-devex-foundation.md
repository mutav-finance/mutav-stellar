# SGR Stellar Devex Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land Apache-2.0 + security disclosure + corrected `CONTRIBUTING.md` + `.editorconfig`/`.gitattributes`/`CODEOWNERS`/PR template on the existing `chore/devex-foundation` branch, in 4 atomic commits.

**Architecture:** Docs and config only. No production code. No CI workflows in this PR (deferred to first Phase A PR per spec). Commits land in **dependency order** so every revision at HEAD has zero dangling file references — a file is committed only after every file it links to already exists at HEAD.

**Tech Stack:** git, GitHub. No build tooling required.

**Spec:** [`docs/specs/2026-05-08-devex-foundation-design.md`](../specs/2026-05-08-devex-foundation-design.md) (committed as `bc4dde8`).

**Branch:** `chore/devex-foundation` (already exists, currently 1 commit ahead of `main` — the spec doc).

**Working tree state at plan time:** three untracked files in repo root (`.editorconfig`, `.gitattributes`, `CONTRIBUTING.md`). These will be **superseded** by content authored in this plan — the plan re-creates the final form rather than editing the in-flight drafts, so each task is self-contained.

---

## Commit order

| # | Subject | New files | Edits |
|---|---|---|---|
| 1 | `chore(license): adopt Apache-2.0` | `LICENSE`, `NOTICE` | `README.md` (append License section) |
| 2 | `chore(security): add disclosure policy` | `SECURITY.md` | — |
| 3 | `chore(devex): add editorconfig and gitattributes` | `.editorconfig`, `.gitattributes` | — |
| 4 | `chore(github): add CONTRIBUTING, CODEOWNERS, PR template` | `CONTRIBUTING.md`, `.github/CODEOWNERS`, `.github/pull_request_template.md` | — |

`CONTRIBUTING.md` lands **last** because it links to `LICENSE`, `SECURITY.md`, `CLAUDE.md` (already exists), and the PR template — all must exist at HEAD when CONTRIBUTING.md is committed.

---

## Task 1: Adopt Apache-2.0

**Files:**
- Create: `/Users/jubs/Projects/mutav-finance/mutav-stellar/LICENSE`
- Create: `/Users/jubs/Projects/mutav-finance/mutav-stellar/NOTICE`
- Modify: `/Users/jubs/Projects/mutav-finance/mutav-stellar/README.md` (append at EOF)

- [ ] **Step 1: Verify branch and clean state**

```bash
cd /Users/jubs/Projects/mutav-finance/mutav-stellar
git branch --show-current
git status
```

Expected: branch is `chore/devex-foundation`. Untracked files (`.editorconfig`, `.gitattributes`, `CONTRIBUTING.md`) are present but not yet staged. `git status` shows nothing staged for commit.

- [ ] **Step 2: Fetch the canonical Apache-2.0 license text**

```bash
curl -fsSL https://www.apache.org/licenses/LICENSE-2.0.txt -o LICENSE
```

Expected: command exits 0, `LICENSE` file is created.

- [ ] **Step 3: Verify LICENSE is the canonical Apache-2.0 text**

```bash
wc -c LICENSE
head -3 LICENSE
tail -1 LICENSE
```

Expected:
- `wc -c` reports `11358 LICENSE` (exact byte count of the canonical apache.org file as of the 2004 publication; if you see a different number within ±100 bytes, inspect manually — apache.org occasionally adjusts trailing whitespace).
- First three lines are blank lines or the indented "Apache License" header. The phrase `Apache License` appears in the first 5 lines.
- Last line ends with `END OF TERMS AND CONDITIONS` *or* a closing line of the appendix; not a stray HTML tag (would indicate the curl hit an error page).

If verification fails, do **not** commit — re-fetch from `https://www.apache.org/licenses/LICENSE-2.0.txt` or copy from another known-good Apache-2.0 repo (e.g. `https://raw.githubusercontent.com/apache/logging-log4j2/main/LICENSE`).

- [ ] **Step 4: Create `NOTICE`**

Write the following file at `NOTICE` (repo root):

```
SGR Stellar
Copyright 2026 TGA Protocol contributors.

This product is part of SGR (Sistema de Garantia Registrada),
an onchain rental-guarantee protocol. See https://github.com/mutav-finance/mutav.
```

- [ ] **Step 5: Append License section to `README.md`**

Open `README.md` and append the following at end-of-file (preserve all existing content):

```markdown

## License

Apache-2.0. See [LICENSE](./LICENSE) and [NOTICE](./NOTICE).
```

Note the leading blank line — required so the new `## License` heading is separated from the previous paragraph (`See [CONTRIBUTING.md]...`).

- [ ] **Step 6: Verify the README edit**

```bash
tail -5 README.md
```

Expected: last 5 lines show the new `## License` heading and its body. The line immediately preceding the heading should be a blank line.

- [ ] **Step 7: Stage and commit**

```bash
git add LICENSE NOTICE README.md
git status
```

Expected: only the three files listed above are staged. The three untracked files in the working tree (`.editorconfig`, `.gitattributes`, `CONTRIBUTING.md`) are NOT staged.

```bash
git commit -m "$(cat <<'EOF'
chore(license): adopt Apache-2.0

Add LICENSE (Apache-2.0 canonical text), NOTICE (copyright +
project attribution), and a License section in README pointing
to both files.

Decision and rationale: see
docs/specs/2026-05-08-devex-foundation-design.md (#License).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: commit succeeds, pre-commit hooks (none currently) pass, `git log --oneline -1` shows the new commit.

---

## Task 2: Add security disclosure policy

**Files:**
- Create: `/Users/jubs/Projects/mutav-finance/mutav-stellar/SECURITY.md`

- [ ] **Step 1: Create `SECURITY.md`**

Write the following at repo root as `SECURITY.md`:

```markdown
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
```

- [ ] **Step 2: Verify the file renders as expected**

```bash
cat SECURITY.md | head -3
wc -l SECURITY.md
```

Expected: file starts with `# Security Policy`. Total line count between 20 and 30 lines.

- [ ] **Step 3: Stage and commit**

```bash
git add SECURITY.md
git status
git commit -m "$(cat <<'EOF'
chore(security): add disclosure policy

Add SECURITY.md routing vulnerability reports through GitHub's
private vulnerability reporting feature, with scope (in/out)
and acknowledgement expectation (5 business days, no fixed SLA).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: commit succeeds.

---

## Task 3: Add editorconfig and gitattributes

**Files:**
- Create: `/Users/jubs/Projects/mutav-finance/mutav-stellar/.editorconfig`
- Create: `/Users/jubs/Projects/mutav-finance/mutav-stellar/.gitattributes`

The current working tree contains untracked draft versions of both files. **Do not** simply `git add` them — re-author from this plan to ensure the trim of the `brand/**` line lands.

- [ ] **Step 1: Author the final `.editorconfig`**

Overwrite `.editorconfig` (or create if missing) with the following exact content:

```ini
root = true

[*]
charset = utf-8
end_of_line = lf
indent_style = space
indent_size = 2
insert_final_newline = true
trim_trailing_whitespace = true

[*.{rs,toml}]
indent_size = 4

[Makefile]
indent_style = tab

[*.md]
trim_trailing_whitespace = false
```

- [ ] **Step 2: Author the final `.gitattributes`**

Overwrite `.gitattributes` (or create if missing) with the following exact content. The line `brand/**       linguist-vendored` from the in-flight draft is **omitted** because no `brand/` directory is vendored in `mutav-stellar`:

```gitattributes
* text=auto eol=lf

# Binary
*.png binary
*.jpg binary
*.jpeg binary
*.gif binary
*.webp binary
*.ico binary
*.pdf binary
*.woff binary
*.woff2 binary

# Linguist hints
*.rs   linguist-language=Rust
*.ts   linguist-language=TypeScript
docs/**       linguist-documentation
.design/**    linguist-vendored
```

- [ ] **Step 3: Verify the trim landed**

```bash
grep -n "brand" .gitattributes || echo "OK: no brand/** line"
```

Expected output: `OK: no brand/** line`. If grep finds anything, re-author Step 2.

- [ ] **Step 4: Verify gitattributes rules apply**

```bash
git check-attr -a README.md
git check-attr -a docs/specs/2026-05-08-devex-foundation-design.md
```

Expected:
- `README.md`: `text: auto`, `eol: lf`.
- The spec doc under `docs/`: `linguist-documentation: set` should appear among the attributes.

- [ ] **Step 5: Stage and commit**

```bash
git add .editorconfig .gitattributes
git status
git commit -m "$(cat <<'EOF'
chore(devex): add editorconfig and gitattributes

EditorConfig sets baseline indent (2 sp; 4 for Rust/TOML; tab for
Makefile) and LF line endings. .gitattributes enforces LF, marks
common binaries, and adds GitHub Linguist hints so docs/ shows as
documentation and .design/ as vendored.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: commit succeeds.

---

## Task 4: Add CONTRIBUTING, CODEOWNERS, and PR template

**Files:**
- Create: `/Users/jubs/Projects/mutav-finance/mutav-stellar/CONTRIBUTING.md`
- Create: `/Users/jubs/Projects/mutav-finance/mutav-stellar/.github/CODEOWNERS`
- Create: `/Users/jubs/Projects/mutav-finance/mutav-stellar/.github/pull_request_template.md`

The current working tree contains an untracked draft `CONTRIBUTING.md`. **Re-author** from this plan rather than editing the draft, so the final form is unambiguous.

- [ ] **Step 1: Create `.github/` directory**

```bash
mkdir -p .github
```

Expected: directory exists. (No-op if already present.)

- [ ] **Step 2: Author the final `CONTRIBUTING.md`**

Overwrite `CONTRIBUTING.md` (or create if missing) with the following exact content:

````markdown
# Contributing to mutav-stellar

Thanks for your interest in contributing.

## Canonical workflow

The protocol-wide branch workflow, commit-message conventions, and PR review process live in the canonical docs repo:

- **General CONTRIBUTING:** https://github.com/mutav-finance/mutav/blob/main/CONTRIBUTING.md

This file documents only `mutav-stellar`-specific notes. If anything below conflicts with the canonical doc, prefer the canonical doc and open an issue here.

## Setup

```bash
git clone https://github.com/mutav-finance/mutav-stellar.git
cd mutav-stellar
git config core.hooksPath .githooks
```

The `.githooks/pre-push` hook blocks direct pushes to `main`. Use a feature branch and open a PR.

## Stack

- **TypeScript** via [Bun](https://bun.sh/) — `@mutav-finance/mutav-stellar` API package (Phase A: Horizon provider).
- **Rust + Soroban SDK** — smart contracts (Phase B, future).

## Repo conventions

- **No barrel files.** Don't create `index.ts` files that only re-export from sibling modules. Public API entry points are declared via `package.json` `exports` map; consumers import from specific subpaths. See [`CLAUDE.md`](./CLAUDE.md).
- **Branch naming:** `feat/...`, `fix/...`, `chore/...`, `docs/...`, `spec/...`.
- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`).
- **PRs:** squash-merge to `main`. The squash commit message follows Conventional Commits.
- **Code style (Rust):** run `cargo fmt` and `cargo clippy --all-targets --all-features -- -D warnings` before pushing.
- **Code style (TypeScript):** formatter choice deferred until needed. Match surrounding code.

## Reporting issues

- **Bugs / features:** open a GitHub issue. We don't use templates yet — a clear title with a reproducer is enough.
- **Security vulnerabilities:** **do not** open a public issue. See [`SECURITY.md`](./SECURITY.md).

## License

Licensed under [Apache-2.0](./LICENSE). Contributions are accepted under the same license per Apache-2.0 §5.
````

Two changes from the in-flight draft:

1. **Reporting issues → "Bugs / features"** line: `"open a GitHub issue using the structured templates."` → `"open a GitHub issue. We don't use templates yet — a clear title with a reproducer is enough."`
2. **License** section: replaced entirely. Old text said license was pending and contributors agreed to "whatever license is ultimately adopted." New text states Apache-2.0 explicitly and references §5 (which auto-licenses contributions under the same terms).

- [ ] **Step 3: Author `.github/CODEOWNERS`**

Write the following at `.github/CODEOWNERS`:

```
* @hoffms
```

(Single line plus trailing newline. Globs `*` to all files; `@hoffms` is the sole maintainer.)

- [ ] **Step 4: Author `.github/pull_request_template.md`**

Write the following at `.github/pull_request_template.md`:

```markdown
## What

<!-- What does this PR do? Keep it short. -->

## Why

<!-- Why is this change needed? Link issues if relevant. -->

## How to test

<!-- Steps to verify this works. -->
```

- [ ] **Step 5: Verify CONTRIBUTING.md links resolve at HEAD**

```bash
# Files referenced by CONTRIBUTING.md:
ls LICENSE SECURITY.md CLAUDE.md .githooks/pre-push
```

Expected: all four files listed without errors. (If any are missing, the dependency-order assumption broke — stop and investigate.)

- [ ] **Step 6: Verify CODEOWNERS syntax**

```bash
cat .github/CODEOWNERS
```

Expected: single line `* @hoffms` followed by a newline. No trailing whitespace.

A formal validator requires GitHub: after pushing the branch, GitHub will surface CODEOWNERS errors at `/<repo>/community` or in the PR review-requesters list. If wanted locally, the [`codeowners-validator`](https://github.com/mszostok/codeowners-validator) tool can be used, but it is not required for this task.

- [ ] **Step 7: Stage and commit**

```bash
git add CONTRIBUTING.md .github/CODEOWNERS .github/pull_request_template.md
git status
git commit -m "$(cat <<'EOF'
chore(github): add CONTRIBUTING, CODEOWNERS, and PR template

CONTRIBUTING.md delegates protocol-wide workflow to the canonical
mutav/CONTRIBUTING.md, documents stellar-specific notes (Bun stack,
no barrel files, code style), and points security reports at
SECURITY.md. CODEOWNERS sets @hoffms as the sole maintainer for
all paths. PR template prompts for What/Why/How to test.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: commit succeeds. `git log --oneline -5` shows all four implementation commits plus the spec commit (`bc4dde8`).

---

## Task 5: End-to-end verification

- [ ] **Step 1: Confirm working tree is clean**

```bash
git status
```

Expected: `nothing to commit, working tree clean`. No untracked files. If `.editorconfig`, `.gitattributes`, or `CONTRIBUTING.md` still appear as untracked, Tasks 3 or 4 missed an overwrite — investigate.

- [ ] **Step 2: Confirm branch is exactly 5 commits ahead of `main`**

```bash
git log --oneline main..HEAD
```

Expected, top-to-bottom (newest first):

```
chore(github): add CONTRIBUTING, CODEOWNERS, and PR template
chore(devex): add editorconfig and gitattributes
chore(security): add disclosure policy
chore(license): adopt Apache-2.0
docs(spec): devex foundation design (Apache-2.0, audience D)
```

- [ ] **Step 3: Walk each commit and verify HEAD@<commit> has zero dangling refs**

```bash
for c in $(git log --reverse --format=%H main..HEAD); do
  git show --stat $c | head -3
  # CONTRIBUTING.md only exists from commit 4; SECURITY.md from commit 2; LICENSE from commit 1
done
```

Expected: each commit subject matches the table in the "Commit order" section. Manual review: at the commit that introduces `CONTRIBUTING.md`, `LICENSE` and `SECURITY.md` and `.githooks/pre-push` and `CLAUDE.md` must already exist in the tree (they do — Tasks 1, 2, plus pre-existing files).

- [ ] **Step 4: Render-check key markdown files**

Visually open in a markdown previewer (or push to a draft PR for GitHub-rendered preview):

- `README.md` — License section appears at the bottom; existing content (Stack, Setup) intact.
- `CONTRIBUTING.md` — links to `LICENSE`, `SECURITY.md`, `CLAUDE.md`, and the canonical mutav CONTRIBUTING all resolve.
- `SECURITY.md` — three sections (Reporting, Scope, Response expectations).
- `LICENSE` — first lines show "Apache License Version 2.0 January 2004"; ends with `END OF TERMS AND CONDITIONS` or the appendix.
- `NOTICE` — single short paragraph; copyright line correct.

- [ ] **Step 5: Push branch and open draft PR**

```bash
git push -u origin chore/devex-foundation
gh pr create --draft --title "chore: devex foundation (Apache-2.0, security policy, contributing)" --body "$(cat <<'EOF'
## Summary

Lands the devex foundation for `mutav-stellar`:

- Adopts **Apache-2.0** (`LICENSE`, `NOTICE`, README License section).
- Adds **`SECURITY.md`** routing reports through GitHub private advisories.
- Adds final **`CONTRIBUTING.md`** delegating to canonical `mutav/CONTRIBUTING.md`.
- Adds **`.editorconfig`** and **`.gitattributes`** (LF, binary marks, Linguist hints).
- Adds **`.github/CODEOWNERS`** (`* @hoffms`) and a **PR template** (What / Why / How to test).

CI is intentionally **not** added in this PR — `mutav-stellar` has no `package.json` or contracts code yet. CI lands with the first Phase A PR.

Audience: future auditors and B2B integrators (see spec).

## Spec

[`docs/specs/2026-05-08-devex-foundation-design.md`](./docs/specs/2026-05-08-devex-foundation-design.md)

## Test plan

- [ ] CI is intentionally absent in this branch — verify GitHub does not surface any failed checks.
- [ ] Verify GitHub auto-detects `LICENSE` (badge appears on repo home, "About" sidebar shows "Apache-2.0 license").
- [ ] Verify GitHub auto-detects `SECURITY.md` ("Security" tab on repo shows the policy).
- [ ] Verify GitHub auto-detects `CODEOWNERS` (the PR's "Reviewers" section auto-suggests `@hoffms`).
- [ ] Verify the PR template populates the body of any new PR opened on this repo.
- [ ] Visually skim rendered README, CONTRIBUTING, SECURITY for broken links.

## Follow-ups (separate issues)

- Adopt Apache-2.0 across mutav, `mutav-solana`, `mutav-app`.
- Convert `mutav-solana/CONTRIBUTING.md` to delegate-to-canonical pattern.
- Wire CI on first Phase A PR (TS/Bun typecheck + test).
- Add per-file SPDX headers when Soroban contracts land (Phase B).
EOF
)"
```

Expected: branch pushed, draft PR opened, URL printed.

- [ ] **Step 6: Inspect the GitHub PR**

Open the PR URL and confirm:

- 5 commits visible in the order from Step 2.
- Files Changed tab shows: 8 new files, 1 modified (`README.md`).
- "Reviewers" auto-suggests `@hoffms` (CODEOWNERS working).
- Repo "About" sidebar (after merge, but visible on the branch view) shows License: Apache-2.0.
- No CI checks running (none defined).

If any of the GitHub-side automatic detections fail (license badge, security policy detection, CODEOWNERS reviewer suggestion), the underlying file is in the wrong location or has a syntax issue — investigate before marking the PR ready.

- [ ] **Step 7: Mark PR ready when verifications pass**

```bash
gh pr ready
```

Plan complete.

---

## Notes for the implementer

- **Do not delete the working-tree drafts of `.editorconfig`, `.gitattributes`, `CONTRIBUTING.md` before starting.** They become superseded by the overwrite steps in Tasks 3 and 4. After Task 4 commits, `git status` will confirm they're tracked. If you delete them before authoring the final form, you'll have to re-create from this plan anyway — no harm, just extra work.
- **No tests exist or run in this PR.** This is a docs/config-only change. The "verify" steps in each task are the equivalent of test runs.
- **The pre-push hook (`.githooks/pre-push`) blocks pushes to `main` by default.** This branch is `chore/devex-foundation` — `git push -u origin chore/devex-foundation` is allowed.
- **Commit messages use Conventional Commits.** Each commit's subject starts with a type (`chore:`, `docs:`) and a scope in parentheses; body is wrapped at ~72 columns; `Co-Authored-By` trailer is required.
