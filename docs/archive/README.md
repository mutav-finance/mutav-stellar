# Project archive

Historical artifacts kept for context and traceability after the live design moved on. **Nothing here describes the current contract or current process** — read the entries linked below only as evidence of what came before.

## When to archive

A document graduates here when **all** of the following are true:

1. It described a design, deployment, or process state that the project has since superseded.
2. Deleting it would lose audit-trail value (e.g., the on-chain artifact still exists; the lessons informed a later decision; the document is referenced from a PR / commit / issue we want readers to be able to follow).
3. Leaving it in its original folder would mislead a casual reader into thinking it describes the current system.

If only (1) is true, prefer **deleting** the file (git history is sufficient). Use the archive when (2) or (3) make deletion risky.

## How to archive

```bash
git mv docs/<old-folder>/<file>.md docs/archive/<file>.md
# Update the document's banner to point at the live successor.
# Add a row to the register below: date archived, why, where to read instead.
```

Cross-references from `docs/archive/` to `docs/specs/`, `docs/architecture/`, and `contracts/` use the same `../` relative paths that worked from the original folder (archive sits at the same depth as `demos/`, `specs/`, etc.), so most internal links survive the move without edits.

## Register

| Archived | Original path | Why | Read instead |
| --- | --- | --- | --- |
| 2026-06-10 | `docs/demos/2026-06-09-stage1-vault-testnet-simulation.md` | Pre-pivot walkthrough. The deployed testnet contract at `CAJTKYO...XWAJR` was the richer pre-simplification surface (`pay_default` lifecycle, operator role, `PendingSwap` tracking). After the 2026-06-09 pivot the vault was reduced to ~410 LOC with a single `withdraw` value-flow path; all policy moved to the OZ Smart Account. | [`docs/specs/2026-06-08-stage1-reserve-vault-design.md`](../specs/2026-06-08-stage1-reserve-vault-design.md) for the current design; a post-pivot simulation will land separately once PR #97 merges. |

When you add an entry, keep it short — the document's own banner should carry the detail; this register is for finding things.
