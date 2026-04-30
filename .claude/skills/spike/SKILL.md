---
name: spike
description: Run a Phase 0 verification check from GENESIS §14 and append the finding to docs/spike-results.md. Use when the user asks to spike, verify a Phase 0 assumption, or check carve-outs.
argument-hint: [check-id]
disable-model-invocation: true
allowed-tools: Bash, Read, Write, Edit, Grep, Glob
---

# /spike — Phase 0 verification runner

The user wants to run check `$ARGUMENTS` from GENESIS §14 Phase 0. If `$ARGUMENTS` is empty, list the unverified checks first and ask which to run.

## Procedure

1. Read `GENESIS.md` §14 Phase 0 to find the matching check. If `docs/spike-results.md` already records a `verified` outcome for it, ask the user whether to re-run.

2. Determine the platform (`uname -s`, `uname -m`). Note any constraint that prevents running on this platform (e.g., a Windows-only check on macOS) and record `inconclusive` with reason.

3. Run the check exactly as written in §14. Capture stdout/stderr. **Never modify the user's `~/.claude`, `~/.codex`, or `~/.gemini` directories.** Use `mktemp -d` paths for redirection targets.

4. Append a finding to `docs/spike-results.md` in this format:

   ```markdown
   ## <check-id> — <one-line title from §14>

   - **Platform:** <os> <arch>
   - **Date:** <ISO-8601>
   - **Outcome:** verified | disconfirmed | inconclusive
   - **Evidence:** <commands run, key output, exit codes>
   - **Implication:** <does this match the §7 spec? if disconfirmed, what does GENESIS need to change?>
   ```

5. **If the outcome is `disconfirmed`**, do NOT proceed to write adapter code. Open a doc-update PR per the operating instructions in GENESIS §10. Surface this clearly to the user.

6. If the file `docs/spike-results.md` does not yet exist, create it with the header:

   ```markdown
   # Phase 0 spike results

   Findings for the verification matrix in [GENESIS.md §14 Phase 0](../GENESIS.md). Each entry is one of: verified | disconfirmed | inconclusive.
   ```

## Reminder

GENESIS §10 of operating instructions: "If a Phase 0 check disconfirms an assumption in §7, open a PR that updates this document _before_ writing any adapter code. The doc is the source of truth; never let code drift ahead of it."
