---
name: cut-phase
description: Tag a phase-N-complete commit and open a PR for human review. Use when the user says we are done with a phase and ready to ship it.
argument-hint: <N>
disable-model-invocation: true
allowed-tools: Bash, Read, Edit
---

# /cut-phase — phase boundary release

Wraps up Phase `$ARGUMENTS` per GENESIS §14 / §17 conventions.

## Pre-flight (block on any failure)

1. **Run `/genesis-check`** first. If verdict is `BLOCKED`, refuse to proceed.
2. Verify the working tree is clean: `git status --short` returns empty.
3. Verify the local gate passes:
   ```sh
   cargo fmt --all -- --check && \
   cargo clippy --workspace --all-targets --all-features -- -D warnings && \
   cargo nextest run --workspace --all-features && \
   cargo deny check && typos && cargo machete
   ```
4. Verify the phase exit criteria from GENESIS §14 are all checked off. Read the corresponding subsection.

## Tag and PR

5. On the current `phase-${ARGUMENTS}-*` branch, create an annotated tag:
   ```sh
   git tag -a phase-${ARGUMENTS}-complete -m "Phase ${ARGUMENTS} complete"
   ```
6. Push branch and tag:
   ```sh
   git push -u origin HEAD
   git push origin phase-${ARGUMENTS}-complete
   ```
   (Both pushes go through the `ask` permission gate — wait for user confirmation.)

7. Open the PR with `gh pr create` (also gated). Title: `phase-${ARGUMENTS}: <one-line summary>`. Body must include:
   - **Exit criteria** — bulleted list quoted from GENESIS §14, each with a checkbox marked done and a one-line evidence note.
   - **Spec alignment** — output of `/genesis-check`.
   - **Open notes** — anything in `docs/notes/phase-${ARGUMENTS}-*.md`.
   - **What's next** — pointer to the next phase's first task.

## Pause for review

8. Per GENESIS operating instruction §5: pause for human review before continuing to the next phase. Do not start phase N+1 work until the PR is merged.
