---
name: cut-phase
description: Autonomously close out a phase — tag, push, open a PR with auto-merge enabled. Invoked by /autopilot at phase completion; rarely invoked by humans directly. Use when the user explicitly says "cut phase N" or autopilot reaches phase-complete state.
argument-hint: <N>
disable-model-invocation: true
allowed-tools: Bash, Read, Edit, Agent
---

# /cut-phase — autonomous phase boundary

Wraps up Phase `$ARGUMENTS` and hands the PR to GitHub auto-merge. Designed to run inside `/loop /autopilot`; produces no prompts.

## Pre-flight (halt on any failure)

1. **Working tree clean:** `git status --porcelain` returns empty. If not, halt with `dirty-tree`.
2. **On the right branch:** `git symbolic-ref --short HEAD` returns `phase-${ARGUMENTS}-implementation` (or `phase-${ARGUMENTS}-spike` for Phase 0). If not, halt with `wrong-branch`.
3. **Local gate green:** invoke `/check`. Every stage must PASS. If any fails, halt with `gate-failure-pre-cut` and the failing stage name. Do NOT attempt to fix at this stage; that was the per-task loop's job.
4. **Spec aligned:** invoke the `genesis-validator` subagent. Verdict must be `READY` or `READY-WITH-NOTES`. `BLOCKED` halts with `genesis-validator-blocked`.
5. **Phase exit criteria met:** read GENESIS §14 for the phase. Verify each criterion has evidence (a commit, a `docs/spike-results.md` entry, a passing test). If any is unmet, halt with `exit-criteria-unmet` and list the missing items.

## Tag and push

6. `git tag -a phase-${ARGUMENTS}-complete -m "Phase ${ARGUMENTS} complete"`.
7. `git push origin phase-${ARGUMENTS}-implementation`.
8. `git push origin phase-${ARGUMENTS}-complete`.

## Open the PR

9. Build the PR body from a heredoc:

   ```markdown
   ## Phase ${ARGUMENTS} — <title from GENESIS §14>

   Closes the phase per [GENESIS §14](../GENESIS.md).

   ### Exit criteria

   <one bullet per criterion from §14, each with a checkbox marked done and evidence (commit short or file ref)>

   ### Spec alignment

   <paste the latest genesis-validator verdict block>

   ### Notes

   <list `docs/notes/phase-${ARGUMENTS}-*.md` files if any>

   ### Auto-merge

   This PR has auto-merge enabled. It will land on `main` when CI is green.
   ```

10. `gh pr create --base main --head phase-${ARGUMENTS}-implementation --title "phase-${ARGUMENTS}: <title>" --body-file <heredoc> --label auto-merge`. Capture the PR number.

## Enable auto-merge

11. `gh pr merge <num> --squash --auto`. This is idempotent if already enabled.

12. Print a one-line status:

    ```
    [CUT] Phase ${ARGUMENTS} PR #<num> opened, auto-merge enabled. Awaiting CI.
    ```

13. Exit. autopilot's next `/loop` fire enters `pr-pending` state and polls.

## Boundaries

- **Don't merge manually.** Even if CI is already green, use `--auto` so the merge is auditable.
- **Don't push directly to main.** Auto-merge will. Branch protection should reject direct main pushes; if it doesn't, fix the protection.
- **Don't approve your own PR.** If the repo requires approvals to auto-merge, that's a configuration issue the human resolves once.
- **Don't tag a phase you didn't fully validate.** The pre-flight checks are load-bearing; halts are how the design stays honest.
