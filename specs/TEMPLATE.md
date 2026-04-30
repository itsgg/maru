# <Spec title>

> **Status:** draft | accepted | implemented | superseded
> **Phase:** N (per [GENESIS.md §14](../GENESIS.md))
> **Owner:** @<github-handle>
> **Created:** YYYY-MM-DD
> **Last updated:** YYYY-MM-DD

## What

One paragraph. The thing this spec describes, at the level a teammate could repeat back to you in a hallway.

## Why

One paragraph. The user problem or constraint motivating this. Reference the relevant GENESIS section if applicable. If this conflicts with GENESIS, say so explicitly — and open the GENESIS-update PR first.

## Acceptance criteria

A checklist that decides "done." Each item is observable from outside the implementation.

- [ ] …
- [ ] …
- [ ] …

## Files affected

Bulleted list of files this spec will touch. Use `path/to/file.rs:NN` when you can.

- `crates/maru-core/src/...`
- `crates/maru-adapters/src/...`
- `docs/...`

## Dependencies

Other specs, ADRs, or upstream changes this depends on. If none, write "none."

- depends on: #<issue or spec>
- blocked by: …

## Verification

How we'll know it works. The exact commands, fixtures, or manual steps. Distinguish "passes locally" from "passes in CI" from "verified against the real harness."

```sh
cargo nextest run -p maru-core profile_name::
```

## Notes / open questions

Anything you want a reviewer to weigh in on. Delete this section before merging if it's empty.
