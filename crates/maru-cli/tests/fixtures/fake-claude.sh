#!/usr/bin/env bash
# Fake `claude` binary for e2e tests. Prints the env vars maru should
# have set, then exits 0.
set -u
echo "FAKE_CLAUDE_CALLED"
echo "CLAUDE_CONFIG_DIR=${CLAUDE_CONFIG_DIR:-<unset>}"
echo "CLAUDE_CODE_PLUGIN_CACHE_DIR=${CLAUDE_CODE_PLUGIN_CACHE_DIR:-<unset>}"
echo "argv=$*"
exit 0
