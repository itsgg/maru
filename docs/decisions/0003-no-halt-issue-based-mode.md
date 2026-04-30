# 0003 — No-halt mode: issues replace halts, autopilot edits GENESIS

- **Status:** accepted
- **Date:** 2026-04-30
- **Deciders:** @itsgg
- **Tags:** process, automation
- **Supersedes:** [0002](0002-autonomous-implementation-mode.md) on the question of "what does autopilot do when it can't proceed."

## Context

ADR 0002 established autonomous merge: `/loop /autopilot` runs phases without per-PR review. But it preserved a halt model: when autopilot couldn't proceed (Phase 0 disconfirmation, dep-budget violation, scope conflict, gate failure), it stopped the loop and waited for a human.

The owner has elected to remove every halt. The loop must keep running unattended for days, surviving any single problem by either auto-recovering or routing around it. This ADR records that decision and the new design.

## Decision

**Halts are replaced by GitHub issues + skip-and-continue.** Operational failures auto-recover. Design failures open an `autopilot:<category>` issue, mark the affected TODO `[BLOCKED:#nn]`, and the loop continues with the next eligible task. When all eligible work is exhausted, autopilot exits cleanly so the `/schedule`'d cron resumes later.

**autopilot autonomously edits GENESIS.md** for clearly-corrective changes (Phase 0 disconfirmations contradicting §7, dep-budget additions when a documented section requires a dep, scope additions when a phase exit criterion needs a missing TODO). Each spec edit:

- lands as a separate PR titled `docs/genesis: ... [auto-edit]`,
- references the issue or finding that prompted it,
- auto-merges on green CI,
- is followed by a retry of the originally-blocked task.

Autonomous spec edits are **bounded** — autopilot does not modify:

- §1 (Mission)
- §4 (Architecture, including the layered design)
- §6 (Core types) — except adding new derives or trait methods that GENESIS itself signals as TBD
- The forbidden dependency list in §13 (`tokio`, `async-std`, `reqwest`, web frameworks, ORMs)
- Safety constraints (`unsafe_code = "deny"`, credential deny-list, the Linux/WSL Claude credential gate from §7.1)
- Any explicit "non-goal" in §2

For changes inside that bounded set, autopilot opens an `autopilot:spec-amendment-needed` issue and waits.

## Consequences

**Positive:**

- The owner can sleep, vacation, or vanish for a week. The loop chews through the backlog.
- Issues become the contract: every "thing autopilot couldn't do" is a tracked, reviewable, closeable artifact.
- Closing an issue automatically retries the corresponding task on the next iteration.
- Spec-correcting drift is auto-resolved when the model has high confidence (matching documented §7 behavior to verified §14 spike findings).

**Negative:**

- The owner's leverage point shrinks to "review merged commits and open issues." A model misinterpretation can land before anyone notices.
- Autonomous spec edits are powerful and risky. The bounded set above is the only safety net; if a misjudged edit slips through, it can land on `main` via auto-merge.
- Issues can pile up indefinitely. If the owner doesn't triage, the loop progressively starves and stops.
- Cross-OS Phase 0 spike checks can't run on a single laptop; many findings will be `inconclusive` rather than `verified`.

**Neutral:**

- Reversibility: revert the autopilot SKILL.md to its 0002 form, revert the permissions changes, and the loop returns to the halt model. The autonomy is layered, not foundational.

## Alternatives considered

### A. Stay with ADR 0002's halt model

Rejected by owner explicitly. The user's stated goal is a system that runs unattended for nights and weekends.

### B. No-halt but no autonomous spec edits

The middle position: convert halts to issues, but require human spec-update PRs. Rejected because Phase 0 disconfirmations would block large amounts of dependent work for hours-to-days while the owner is asleep, defeating the point.

### C. Full autonomy including unrestricted spec edits

The most permissive option: autopilot can edit any part of GENESIS, change the dependency budget freely, modify safety constraints. Rejected because the spec stops being load-bearing — the architecture has no anchor a future contributor (or future-self) can rely on, and a single hallucination can ship credential-leaking code.

The chosen option (B's continuity + bounded spec edits) is the reasoned compromise.

## Implementation

- `.claude/skills/autopilot/SKILL.md` — rewritten as no-halt state machine.
- `.claude/skills/cut-phase/SKILL.md` — unchanged from 0002.
- `.claude/hooks/block-dangerous.sh` — permits force-push to `phase-*` branches (autopilot rebases them); main/master still protected.
- `.claude/settings.json` — adds `gh label`, `gh issue reopen/comment`, `gh pr reopen`, `gh repo view` to the allow-list.
- `docs/notes/handoff.md` — updated to reflect issue-based blocks and cron resumption.

## References

- ADR 0002 (auto-merge mode).
- `.claude/skills/autopilot/SKILL.md` "GENESIS-edit policy" and "Issue-based blocking" sections.
- GENESIS.md operating instructions §3 (no longer fully applies; this ADR carves out the autonomous-edit case).
