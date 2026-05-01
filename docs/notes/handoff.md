# Operator handoff — autonomous mode (no-halt + cron)

> **STATUS: archived — phases 0–4 complete.** This file is operator-mode for the now-defunct `/autopilot` cron loop. It is preserved for historical reference. Post-merge user actions for the current state of the project live in [`phase-4-handoff.md`](phase-4-handoff.md).

This is the one-pager. The loop runs unattended; you triage open issues at your own cadence.

## What's running

When the owner started the loop:

- `/autopilot` is in **no-halt mode** (per [ADR 0003](decisions/0003-no-halt-issue-based-mode.md)). Operational failures auto-recover; design failures become GitHub issues with `autopilot:<category>` labels and the loop continues with other work.
- A `/schedule`'d cron fires `/autopilot` every hour. Even when your laptop is closed, scheduled remote runs make progress (provided GitHub credentials are configured for the routine).
- Auto-merge is enabled on `main`. PRs land when CI is green; autopilot doesn't approve, you don't either — the auto-merge runner does.
- autopilot **does** autonomously edit GENESIS.md for bounded corrective changes (Phase 0 spike findings, dep additions when a §-required by spec, scope additions for missing exit-criteria TODOs). The bounded set is documented in ADR 0003.

## What you should look at when you wake up

In order:

```sh
cd /Users/gg/Work/GG/maru
git pull
git log --oneline -20                           # recent merges
gh pr list --state merged --limit 20            # what landed via auto-merge
gh issue list -l "autopilot:*" --state open     # what's blocked
cat TODO.md                                     # current phase progress
ls docs/spike-results.md docs/notes/            # findings, halt notes (rare in no-halt mode)
```

If `gh issue list -l "autopilot:*" --state open` is empty: nothing is blocked, everything is progressing or done.

If it's non-empty: each issue has a category, a what-was-attempted, a what's-blocking, and a "what needs to happen for autopilot to retry." Resolve at your pace; closing the issue triggers a retry on the next cron fire.

## Categories of open issue and what to do

| Label                                    | What it means                                                  | Your move                                                      |
| ---------------------------------------- | -------------------------------------------------------------- | -------------------------------------------------------------- |
| `autopilot:gate-failure`                 | Local gate failed 5× on a single check.                        | Pull the branch, fix the issue manually, push. Close the issue. |
| `autopilot:phase-0-disconfirmation`      | A spike check disconfirmed a GENESIS §7 assumption that autopilot didn't auto-correct (touched a bounded section per ADR 0003). | Read the issue. Write the GENESIS-update PR. Close the issue. |
| `autopilot:dep-budget-violation`         | A task needs a dep autopilot can't autonomously add (the bounded list in ADR 0003 forbids modifying §13's deny-list). | Discuss; either rewrite without the dep or amend §13 manually. |
| `autopilot:out-of-scope`                 | Task can't be done without expanding scope autopilot can't authorize. | Decide whether to expand the spec or narrow the task.          |
| `autopilot:genesis-validator-blocked`    | The validator subagent returned BLOCKED.                       | Read the validator's reasoning. Fix the implementation or the spec. |
| `autopilot:pr-ci-failure`                | PR CI is red after autopilot's retry attempts.                 | Pull, reproduce locally, fix, push.                            |
| `autopilot:rebase-conflict`              | Auto-rebase failed.                                            | `git checkout phase-N-<suffix>`, `git rebase main`, resolve, force-push. Close issue. |
| `autopilot:human-closed-pr`              | Someone (probably you) closed an autopilot PR.                 | If intentional, leave the issue closed. If accidental, re-open the PR. |
| `autopilot:spec-amendment-needed`        | autopilot wants to edit a bounded GENESIS section.             | Review the proposed edit in the issue body. Apply it manually. |

## What autopilot will NOT do

- Push directly to `main`.
- Edit GENESIS §1, §4, §6 (without explicit additions signaled by the spec), or §13's forbidden-deps list.
- Disable lints, gates, or hooks.
- Approve, comment, or close PRs other than its own.
- Run `cargo publish` or `cargo install`.
- Touch your real `~/.claude`, `~/.codex`, or `~/.gemini` directories.

## What you'll see at "all done"

When Phase 4 PR merges, autopilot enters the `all-phases-complete` terminal state:

- Tears down the `/schedule` cron.
- Opens a final `final-handoff` PR updating `CHANGELOG.md` to mark `[1.0.0-alpha.0]` released.
- The PR body lists what external verification is still needed: `brew install`, `scoop install`, `winget install`, `curl | sh`. These can't be CI-tested end-to-end; you verify manually before tagging the release.

## Restarting after laptop sleep / session end

The cron handles this. If you want to manually resume in an interactive session:

```sh
cd /Users/gg/Work/GG/maru
# Open a fresh Claude Code session here, then:
/loop /autopilot
```

State is read from `TODO.md`, open issues, and git tags. There's no in-memory state that gets lost across sessions.

## Killing the loop

Three ways, increasing in scope:

1. **Stop the current /loop in the active session:** end the Claude Code session. The cron continues.
2. **Stop the cron:** open Claude Code, `/schedule list`, find the autopilot routine, `/schedule remove <id>`.
3. **Hard stop everything:** disable auto-merge in GitHub Settings → General. Pending PRs stop merging. Manually close pending PRs.

## Reverting to manual mode

See ADR 0002 ("revertibility" note) and ADR 0003. Three small edits and the halt model is back.
