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

- **No barrel files.** Don't create `index.ts` files that only re-export from sibling modules. Public API entry points are declared via `package.json` `exports` map; consumers import from specific subpaths.
- **Branch naming:** the canonical prefixes (`feat/`, `fix/`, `chore/`, `docs/`, `refactor/`, `test/`) plus `spec/` for specification-only branches in this repo.
- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`).
- **PRs:** squash-merge to `main`. The squash commit message follows Conventional Commits.
- **Code style (Rust):** run `cargo fmt` and `cargo clippy --all-targets --all-features -- -D warnings` before pushing.
- **Code style (TypeScript):** formatter choice deferred until needed. Match surrounding code.

## Reporting issues

- **Bugs / features:** open a GitHub issue. We don't use templates yet — a clear title with a reproducer is enough.
- **Security vulnerabilities:** **do not** open a public issue. See [`SECURITY.md`](./SECURITY.md).

## License

Licensed under [Apache-2.0](./LICENSE). Contributions are accepted under the same license per Apache-2.0 §5.
