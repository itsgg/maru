#!/usr/bin/env bash
# SessionStart hook. Inject git context so the agent knows what state the
# repo is in without having to ask.
set -u

cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0

branch=$(git symbolic-ref --short HEAD 2>/dev/null || echo "no-git")
dirty=$(git status --porcelain 2>/dev/null | wc -l | tr -d ' ')
last=$(git log -1 --pretty=format:'%h %s' 2>/dev/null || echo "no commits")

# Detect current phase from recent tags.
phase=$(git tag --list 'phase-*-complete' --sort=-creatordate 2>/dev/null | head -n1 | sed 's/-complete//' || echo "phase-0")
[[ -z "$phase" ]] && phase="phase-0 (no phases complete)"

ctx="Branch: ${branch}
Dirty files: ${dirty}
Last commit: ${last}
Most recent phase tag: ${phase}
Spec: GENESIS.md is normative. Conventions: CLAUDE.md."

jq -n --arg c "$ctx" \
  '{hookSpecificOutput:{hookEventName:"SessionStart",additionalContext:$c}}'
