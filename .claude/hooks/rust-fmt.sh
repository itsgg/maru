#!/usr/bin/env bash
# PostToolUse hook for Edit/Write/MultiEdit. Auto-format the .rs file just touched.
# Silent on success; never blocks.
set -u

input=$(cat)
file=$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty')
[[ -z "$file" ]] && exit 0
[[ "$file" == *.rs ]] || exit 0
[[ -f "$file" ]] || exit 0

rustfmt --edition 2024 "$file" 2>/dev/null || true
exit 0
