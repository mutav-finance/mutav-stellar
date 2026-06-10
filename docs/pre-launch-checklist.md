# Pre-launch checklist

Single-source register of mainnet-readiness items for this repo's on-chain surface (`reserve_vault` + `fund` contracts + published TS SDK). Update as items land — the launch decision should be auditable from this file alone.

Treat every unchecked box as a blocker. Items here are deliberately not auto-checked by tooling; the act of checking the box is a human attestation.

---

## 1. Key custody — regenerate, do not migrate

The testnet keypairs in `local.env` and `~/.config/stellar/identity/` are **explicitly disposable** (per the header in `local.env.example`). For mainnet, treat them as **tainted** and generate fresh keys; no migration path.

- [ ] **Admin keypair** generated offline; loaded into the hardware wallet managed inside [`mutav-app/apps/admin/`](https://github.com/mutav-finance/mutav-app). Never typed, never on disk in plaintext.
- [ ] **Operator keypair** generated inside the KMS-backed Convex Action on `mutav-app`. Never exported.
- [ ] **Treasury / `protocol_addr`** regenerated (passive recipient, never signs).
- [ ] **Issuer of any classic-asset wrapper** (e.g., `MUTAV-COL`) — generated from cold wallet; `AUTH_REQUIRED` + `AUTH_REVOCABLE` + `AUTH_CLAWBACK_ENABLED` flags set **before** any account holds the asset (the clawback flag is immutable per balance once set).
- [ ] **Old testnet keys swept** — any residual XLM moved to a sink address; the testnet contract at `CBEUN2QV…JQ6R` (and any earlier deployment) explicitly retired.

## 2. Repo / process hardening

- [ ] **Pre-commit secret scanner** active (`.githooks/pre-commit`). Verify by attempting a commit with a fake `S` + 55 base32 chars and confirming rejection. Confirm `git config core.hooksPath .githooks` is set on every contributor's clone.
- [ ] **Separate `.stellar/mainnet.json`** for the mainnet deployment manifest. CI / scripts select by env var; never share the testnet file.
- [ ] **`CODEOWNERS`** covers `.stellar/`, `contracts/`, `src/`, `.github/workflows/`, and `.githooks/`. Changes in those paths require a named reviewer.
- [ ] **GitHub Actions secrets policy** — if mainnet deploy is automated, scope secrets to the deploy job only (job-level `env`). Production branch protection requires a manual approval gate.
- [ ] **No `local.env` ever committed** — `git log --all --diff-filter=A --name-only | grep -E "^local\.env$"` returns empty. Re-confirm before launch.

## 3. Contract verification

- [ ] **External audit complete** on `reserve_vault` + `fund` WASM. Findings closed or explicitly accepted with rationale logged in `docs/architecture/decisions/`.
- [ ] **Constructor admin arg** at deploy points at the hardware-wallet address, not a CLI keypair. Verify the deploy command:
      ```
      stellar contract deploy \
        --wasm target/wasm32v1-none/release/mutav_reserve_vault.wasm \
        --source <deployer-account> \
        --network mainnet \
        -- --admin <hardware-wallet-public-key>
      ```
- [ ] **OZ Smart Account at the admin address** is deployed and configured with: per-signer thresholds, per-asset spending caps, timelocks for high-value withdraws, differential rules for governance vs. value-flow operations. The vault's policy guarantees are properties of the Smart Account — verify they are real before cutover.
- [ ] **Permissionless `extend_ttl` cadence** documented. Either a runbook owner calls it inside the `INSTANCE_TTL_THRESHOLD` window (~6 days at 5s ledgers), or regular admin traffic guarantees the bump as a side effect. Decide which.
- [ ] **CAP-46-6 sanction response runbook** exists — written procedure for `force_remove_approved_asset` if a SEP-41 issuer freezes the vault address. Drill it once on testnet.
- [ ] **Pause runbook** active (see PR [#96](https://github.com/mutav-finance/mutav-stellar/pull/96)).
- [ ] **Vulnerability disclosure runbook** active (see PR [#94](https://github.com/mutav-finance/mutav-stellar/pull/94)).

## 4. Mainnet pre-flight simulation

- [ ] **Post-pivot testnet simulation** of the current WASM is passing — fresh deploy + 10 scenarios green (deploy, allowlist setup, withdraw happy path, reject unapproved asset, reject non-whitelisted destination, pause/unpause, permissionless `extend_ttl`, two-step handover with expiry, `force_remove_approved_asset`, negative remove with nonzero balance).
- [ ] **WASM hash recorded** in `.stellar/mainnet.json` matches the audited build artifact (`cargo build --target wasm32v1-none --release`). Bit-for-bit reproducibility check.
- [ ] **CI is green** on the mainnet-deploy commit. Workflow status link archived alongside the deploy record.

## 5. Boundary verification (workspace-wide invariants)

These are CLAUDE.md-mandated boundary rules. Verify pre-launch, not just at code-review time.

- [ ] **No signing-key code in `mutav-stellar`** — grep returns empty:
      ```
      git grep -nE 'Keypair\.fromSecret|signTransaction\(|_SECRET' -- 'src/**' 'scripts/**'
      ```
      (CI matches in tests or local-env templates are expected — read each hit.)
- [ ] **Dependency direction holds** — `mutav-stellar` does not import from `mutav-app`. The SDK consumers do, not the reverse.
- [ ] **Brand assets under `branding/**`** (if present) are vendored from the `brand/` repo, not edited in-place.

## 6. Sign-off

- [ ] Protocol team lead approval logged in `docs/architecture/decisions/` with the deploy commit SHA.
- [ ] Audit firm approval letter archived under `docs/audits/` (create the folder if this is the first one).
- [ ] Mainnet deploy authorization issued as a recorded multisig operation on the hardware-wallet-backed admin account.

---

## How to use this file

- File lives at `docs/pre-launch-checklist.md`. Linked from the top-level [README.md](../README.md) launch section.
- When an item lands, edit the box from `[ ]` to `[x]` in the same PR that lands the work. PR description should cite the section number.
- Don't auto-check from CI — checking is a human attestation, not a build pass.
- If you discover a new pre-launch item mid-flight, add it under the right section and reference the discovery PR/issue inline.

## Related

- `docs/archive/README.md` — historical / pre-pivot context
- `docs/architecture/02-actors-and-trust.md` — trust model the OZ Smart Account configuration must enforce
- `local.env.example` — the disposable-testnet-keys policy this checklist supersedes for mainnet
