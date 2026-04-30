# Operator handoff — autonomous mode

This is the one-pager for kicking off autonomous implementation of `maru`. Bookmark it.

## One-time setup on GitHub

Done from the repo's web UI (or via `gh api` once authed):

1. **Branch protection on `main`** — Settings → Branches → Add rule for `main`:
   - ☑ Require a pull request before merging.
   - ☑ Require status checks to pass before merging. Add: `fmt`, `clippy`, `typos`, `deny (advisories)`, `deny (bans licenses sources)`, `audit`, `msrv`, `machete`, `test (ubuntu-22.04)`, `test (macos-14)`, `test (windows-latest)`.
   - ☑ Require branches to be up to date before merging.
   - ☐ Require approvals (leave off — autopilot is the only contributor).
   - ☑ Do not allow bypassing the above settings.

2. **Auto-merge enabled** — Settings → General → Pull Requests:
   - ☑ Allow squash merging.
   - ☑ Allow auto-merge.

3. **Default merge method** — Settings → General → Pull Requests → Allow squash merging only (uncheck the other two if you want a clean log).

4. **Verify `gh` is authed:** `gh auth status` should show "Logged in to github.com". If not: `gh auth login`.

## Starting the loop

Open a fresh Claude Code session in `/Users/gg/Work/GG/maru` and run:

```
/autopilot --bootstrap
```

This generates `TODO.md` for Phase 0 from GENESIS §14 and commits it. Inspect the file. If it looks right:

```
/loop /autopilot
```

That's it. The loop runs until something genuinely halts.

## What you'll see

Each `/loop` iteration prints one short status line. Examples:

```
[0.1] docs(spike): record CLAUDE_CONFIG_DIR check on macOS arm64 — committed a3f9b21, pushed
gate: 6/6 PASS
next: [0.2] verify CLAUDE.md user memory carve-out
```

```
[PR-PENDING] phase-0 PR #1 — checks PENDING (4/11). Sleeping 5min.
```

```
[PHASE-COMPLETE] Phase 0 PR #1 MERGED at 2026-04-30T18:42Z.
syncing main, branching phase-1-implementation.
```

```
[HALT] phase-0-disconfirmation on task 0.4 — see docs/notes/autopilot-halt-2026-04-30.md
```

## Halts: what they mean and what to do

| Reason                          | What happened                                            | Your move                                                                            |
| ------------------------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `phase-0-disconfirmation`       | A spike check found GENESIS §7 is wrong about something. | Read the halt note. Open a GENESIS-update PR per the note. Merge. Restart `/loop`.   |
| `dep-budget-violation`          | A task needed a dep outside GENESIS §13.                 | Either rewrite without the dep or amend GENESIS §13 in a separate PR.                |
| `gate-failure`                  | Local gate failed 3× on a single check.                  | Read the halt note. Fix the underlying issue manually. Restart.                      |
| `genesis-validator-blocked`     | Spec drift the validator refused.                        | Read the validator's verdict in the halt note. Fix or update the spec. Restart.      |
| `pr-ci-failure`                 | An auto-merge PR's CI is red on the remote.              | Pull the branch, reproduce locally, fix, push. Auto-merge will finish the merge.     |
| `pr-conflict-needs-rebase`      | `main` moved during the wait.                            | Pull, rebase the phase branch, force-push. Auto-merge will finish.                   |
| `pr-stuck`                      | PR open >3h without merging.                             | Investigate manually (CI runner shortage? required check missing?).                  |
| `dirty-tree`                    | Working tree dirty at iteration start.                   | Investigate: was something else modifying the repo? Clean up, restart.               |
| `out-of-scope`                  | Task can't be done as specified.                         | Read the halt note. Either narrow the task or update the spec.                       |
| `all-phases-complete`           | Phase 4 PR merged. maru is shipped.                      | Verify the install one-liners (brew, scoop, winget, curl). Tag a release.            |

After resolving any halt: delete the halt note (or move it to `docs/notes/resolved/`), then start a new session with `/loop /autopilot`.

## What autopilot will NOT do

- Push directly to `main` (always via PR).
- Modify `GENESIS.md` (a halt asks you to do that yourself).
- Disable lints, gates, or hooks to make a task pass.
- Approve, comment on, or close PRs other than the one it just opened.
- Run `cargo publish` or `cargo install` (not in the allow-list).
- Touch `~/.claude`, `~/.codex`, or `~/.gemini` directories on your machine. Phase 0 spikes use `mktemp -d` exclusively.

## Auditing as you go

Even with auto-merge, you should look in periodically:

```sh
git pull
git log --oneline main..HEAD     # if you're on a phase branch
gh pr list --state merged --limit 20
ls docs/notes/                    # halt notes + reviewer flags
cat TODO.md                       # current progress
```

A weekly skim of merged PRs is a healthy minimum. The /autopilot loop is fast but not infallible.

## Restarting after a halt

```
# In the repo
cat docs/notes/autopilot-halt-*.md   # read the most recent
# … resolve the halt …
rm docs/notes/autopilot-halt-*.md    # or move to resolved/
# In a fresh Claude Code session
/loop /autopilot
```

The loop picks up from wherever the state machine lands.

## Reverting to manual mode

If at any point you want the per-PR human review back: see the "reversibility" note in [`docs/decisions/0002-autonomous-implementation-mode.md`](decisions/0002-autonomous-implementation-mode.md). It's three small edits.
