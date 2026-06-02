# References

Annotated bibliography for the [Mutav Guarantees Vault whitepaper](../whitepaper.md).

One file per source. Each file follows the same shape:

```markdown
# {Source title}

- **Canonical URL:** {primary source}
- **Type:** {standard / paper / production system / incident report / spec}
- **First read:** {date}
- **Cited from:** {MGV whitepaper §N, other docs}

## Summary
{1–2 sentences}

## Relevance to MGV
{Why this source matters for a vault on Soroban with NAV-appreciating shares,
async redemption queue, and operator-mediated yield / default accounting}

## Annotated takeaways
{Bullet list of key ideas, each with a 1–3-sentence note explaining how it
applies to MGV — agree, disagree, adapt, reject}

## Notable quotations
{Direct quotes that load-bear in the whitepaper. Each quote pinned to a URL
fragment or page reference}

## Cross-references
{Other entries in `docs/references/` that this source connects to}
```

## Why one-file-per-source

- **Portable**: other docs can link to the same notes without duplicating them
- **Reviewable**: an auditor reading the whitepaper can drop into any single
  reference without scrolling through prose
- **Searchable**: per-source files are easier to grep + diff than a single
  bibliography
- **Maintainable**: when a source updates (e.g. Soroban CAP edits, ERC4626
  errata, Enzyme version bump), only one file changes
