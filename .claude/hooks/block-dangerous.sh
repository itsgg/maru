#!/usr/bin/env bash
# PreToolUse hook for Bash. Blocks destructive commands and force-pushes to main.
# Inputs (stdin): JSON with .tool_input.command
# Outputs (stdout): JSON permissionDecision when blocking; silent otherwise.
set -u

input=$(cat)
cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // empty')
[[ -z "$cmd" ]] && exit 0

# Patterns that should never run, even if the model asks.
deny_re='(^|[[:space:]])rm[[:space:]]+-rf[[:space:]]+(/|~|\$HOME)([[:space:]]|$)'
deny_re+='|git[[:space:]]+push[[:space:]]+(--force|-f)([[:space:]]|$).*[[:space:]](main|master)([[:space:]]|$)'
deny_re+='|git[[:space:]]+push[[:space:]]+(--force|-f)[[:space:]]+origin[[:space:]]+(main|master)([[:space:]]|$)'
deny_re+='|git[[:space:]]+reset[[:space:]]+--hard[[:space:]]+(origin/)?(main|master)([[:space:]]|$)'

if printf '%s' "$cmd" | grep -qE "$deny_re"; then
  jq -n --arg c "$cmd" \
    '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",permissionDecisionReason:("maru policy: refused to run destructive command: " + $c)}}'
  exit 0
fi

exit 0
