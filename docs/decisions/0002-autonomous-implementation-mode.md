# 0002 — Autonomous implementation mode

- **Status:** accepted
- **Date:** 2026-04-30
- **Deciders:** @itsgg
- **Tags:** process, automation

## Context

GENESIS.md operating instruction §5 originally required a human review pause at every phase boundary. The owner has elected to operate `maru` in fully autonomous mode: `/loop /autopilot` runs phase after phase without manual intervention, halting only on real problems (gate failure, spec disconfirmation, dep-budget violation, CI red).

The trade-off is explicit. Going hands-off on phase reviews means:

- Phase 0 spike findings land via auto-merge, not via human spot-check.
- Phase 1 → 4 work proceeds without a human reading every PR before it merges.
- The blast radius of a model misinterpretation expands — many commits across many phases can land before anyone notices.

The countervailing safeguards we keep:

- The local gate is run by autopilot **before every commit**, with a hard 3-attempt cap and a halt on any failed stage.
- Every PR runs the full CI matrix (fmt, clippy, typos, deny, audit, msrv, machete, test on 3 OSes). Auto-merge fires only on a clean `mergeStateStatus`.
- The `genesis-validator` subagent runs at every adapter task and at every phase cut. A `BLOCKED` verdict halts.
- Phase 0 disconfirmation is a non-negotiable halt: GENESIS must change before adapter code, by a human.
- `dep-budget-violation` halts. Smuggling dependencies in is impossible without a halt note.
- Branch protection on `main` rejects direct pushes. Auto-merge is the only path.
- Halts write `docs/notes/autopilot-halt-<date>.md` so a returning human sees state without reading the chat log.

## Decision

We operate autopilot in continuous-merge mode by default. The flow is:

1. `/loop /autopilot` runs in a fresh session.
2. autopilot picks tasks, implements, validates, commits, and pushes to a per-phase branch.
3. At phase completion, `/cut-phase N` opens a PR with `--label auto-merge` and runs `gh pr merge --squash --auto`.
4. Once the PR's CI is green, GitHub merges it automatically.
5. autopilot's next `/loop` fire detects the merge, syncs `main`, and starts the next phase.

The owner takes responsibility for:

- Configuring GitHub branch protection on `main`: required status checks (the CI jobs), required PR (no direct pushes), allow squash merge, allow auto-merge.
- Reading `docs/notes/autopilot-halt-*.md` when a halt occurs and resolving the halt cause.
- Auditing merged PRs at their own cadence (recommended: at least weekly during active development).

## Consequences

**Positive:**

- maru ships within the GENESIS phase plan without per-PR human bottleneck.
- Halts are surfaced explicitly with written notes; the human's attention is concentrated on real problems, not on review-as-formality.
- Every commit has passed the same gate that CI enforces, so PRs merge on first try in the typical case.

**Negative:**

- The owner must trust the gate and the validator. If the gate is incomplete (e.g., a security check we haven't added), bad code can ship.
- Recovery from a chain of bad merges is harder than catching one bad PR — `git revert` may need to span multiple commits.
- Halts can pile up overnight if the owner is asleep when one fires. Work is paused until the halt is resolved, which can lose a day.

**Neutral:**

- This decision is reversible. To return to per-phase human review: revert the `ask`/`allow` permission moves in `.claude/settings.json`, edit `cut-phase`'s SKILL.md to use `--auto=false`, and update GENESIS operating instruction §5.

## Alternatives considered

### A. Stay with per-phase human review

The original GENESIS §5 model. Safer; slower. Rejected because the owner has explicitly accepted the trade-off and wants throughput.

### B. Auto-merge but no auto-progress to next phase

Auto-merge each PR, but require a human to start the next phase. Halfway position. Rejected as adding friction without adding meaningful safety: if the human trusts auto-merge, they trust the gate, and there's no reason to gate the next phase manually.

### C. Auto-merge only Phase 0 (spike), require review for Phase 1+

Spike findings are low-risk (just markdown). Implementation phases touch real code. Rejected for the same throughput reason; the gate runs the same way regardless of phase.

## References

- GENESIS.md §14 (phase plan), §17 (branching), Operating instructions §5.
- `.claude/skills/autopilot/SKILL.md` — the loop implementation.
- `.claude/skills/cut-phase/SKILL.md` — the auto-merge handoff.
- `.claude/settings.json` — permission allow-list.
