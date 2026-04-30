#!/usr/bin/env bash
# PreToolUse hook for Bash. Blocks destructive commands and risky pushes.
# Inputs (stdin): JSON with .tool_input.command
# Outputs (stdout): JSON permissionDecision when blocking; silent otherwise.
set -u

input=$(cat)
cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // empty')
[[ -z "$cmd" ]] && exit 0

# Patterns that should never run, even if the model asks.
# Force-push to phase-* branches IS allowed (autopilot rebases them); only main/master is protected.
deny_patterns=(
  # rm -rf at home or root
  '(^|[[:space:]])rm[[:space:]]+-rf[[:space:]]+(/|~|\$HOME)([[:space:]]|$)'
  # force-push to main / master with explicit ref
  'git[[:space:]]+push[[:space:]]+(--force|-f|--force-with-lease=?[^[:space:]]*)[[:space:]]+\S+[[:space:]]+(main|master)([[:space:]]|$)'
  # hard-reset main / master (or origin/main)
  'git[[:space:]]+reset[[:space:]]+--hard[[:space:]]+(origin/)?(main|master)([[:space:]]|$)'
  # forceful clean of the working tree
  'git[[:space:]]+clean[[:space:]]+(-fd|-df|-fdx|-dfx)([[:space:]]|$)'
)

reason=""
for re in "${deny_patterns[@]}"; do
  if printf '%s' "$cmd" | grep -qE "$re"; then
    reason="matches dangerous pattern: $re"
    break
  fi
done

# Soft block: bare `git push` while the current branch is main/master.
if [[ -z "$reason" ]] && printf '%s' "$cmd" | grep -qE '^git[[:space:]]+push([[:space:]]|$)'; then
  branch=$(git symbolic-ref --short HEAD 2>/dev/null || true)
  if [[ "$branch" == "main" || "$branch" == "master" ]]; then
    # Allow if user explicitly named another branch.
    if ! printf '%s' "$cmd" | grep -qE 'git[[:space:]]+push[[:space:]]+\S+[[:space:]]+\S+'; then
      reason="bare 'git push' while on $branch — push from a feature branch instead"
    fi
  fi
fi

# Force-push allowed to phase-* branches (autopilot rebases them).
# This block intentionally permissive: anything not denied above is allowed.

if [[ -n "$reason" ]]; then
  jq -n --arg c "$cmd" --arg r "$reason" \
    '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",permissionDecisionReason:("maru policy: " + $r + " — refused: " + $c)}}'
  exit 0
fi

exit 0
