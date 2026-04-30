#!/usr/bin/env bash
# UserPromptSubmit hook. When the prompt looks like a code-writing request,
# inject a one-line reminder of the maru conventions.
set -u

input=$(cat)
prompt=$(printf '%s' "$input" | jq -r '.prompt // empty')
[[ -z "$prompt" ]] && exit 0

# Trigger only on prompts that look like new code work.
if printf '%s' "$prompt" | grep -qiE '(add|implement|write|create|build|fix|refactor).*(function|fn |struct|enum|trait|module|crate|test|adapter)'; then
  jq -n '{hookSpecificOutput:{hookEventName:"UserPromptSubmit",additionalContext:"Reminder: GENESIS.md is the normative spec. anyhow in maru-cli; thiserror in libs; no anyhow in maru-shim. No unwrap/expect outside #[cfg(test)]. Run cargo fmt && cargo clippy -- -D warnings && cargo nextest run before declaring done. New deps require GENESIS §13 justification."}}'
fi
exit 0
