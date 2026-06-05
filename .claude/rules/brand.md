---
name: brand
description: Brand assets are vendored from the brand/ repo. Never edit in place.
paths: branding/**
---

# Brand assets are vendored — don't edit in place

Files under `branding/` are a vendored copy of `brand/branding/` (canonical). Editing them here creates silent drift that `bun brand:audit` will report.

## To update brand artifacts in this repo

1. `cd ../brand`
2. Edit the canonical files under `brand/branding/<key>/`
3. `bun brand:export` — pushes the change to every consumer in `consumers.json`

## To lift a one-off consumer edit back to canonical

1. Make the change here (only as a deliberate one-off)
2. `cd ../brand && bun brand:import mutav-stellar` — pulls this consumer's `branding/` into canonical
3. `bun brand:export` — fans out to the other consumers

`bun brand:audit` (in `brand/`) reports any divergence. If it complains, one of the above flows wasn't followed.
