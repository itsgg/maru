# 0001 — Record architecture decisions

- **Status:** accepted
- **Date:** 2026-04-30
- **Deciders:** @gg
- **Tags:** process

## Context

`maru` has a normative design document ([GENESIS.md](../../GENESIS.md)) that pins architecture, types, and the dependency budget. GENESIS is _normative_, but it's also large and slow to evolve — every change to it requires a spec-update PR before any code changes.

For decisions that don't rise to the level of GENESIS but still need to be remembered (a dependency swap, a logging convention, a deprecation), we need a lighter mechanism.

## Decision

We will keep architecture decision records (ADRs) under `docs/decisions/`, numbered sequentially starting from `0001`. Each ADR uses `0000-template.md` as its starting point.

ADRs are for decisions that:

- affect more than one crate, or
- introduce or remove a workspace dependency, or
- change a convention agents and reviewers will rely on, or
- you'll want to remember the reasoning for in six months.

ADRs are _not_ for:

- anything GENESIS already pins (update GENESIS instead),
- per-feature work plans (those are `specs/` documents),
- "how to do X" runbooks (those are `docs/` pages).

## Consequences

- **Positive:** a durable record of the "why" behind decisions that aren't load-bearing enough for GENESIS. New contributors and future maintainers can read `docs/decisions/` chronologically to catch up.
- **Positive:** ADRs are small enough to review in a single sitting. Pairs well with the `/genesis-check` skill — if a change touches an area an ADR covers, the reviewer is reminded.
- **Negative:** one more place to look when searching for context.
- **Neutral:** ADRs are append-only. Superseding an ADR means writing a new one that links back, never editing the old one in place.

## Alternatives considered

### A. Don't have ADRs; put everything in GENESIS

Rejected. GENESIS is already long and intentionally slow to change. Decisions like "swap `directories` for `etcetera`" don't deserve a GENESIS-update PR but do deserve a record.

### B. Use a wiki

Rejected. Wikis aren't versioned with the code; reviewers won't see them in the PR diff.

## References

- [Documenting Architecture Decisions — Michael Nygard](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
- [adr.github.io](https://adr.github.io/)
