#!/usr/bin/env bash
# Fake `codex` binary for e2e tests. Prints the env vars maru should
# have set, then exits 0.
set -u
echo "FAKE_CODEX_CALLED"
echo "CODEX_HOME=${CODEX_HOME:-<unset>}"
echo "argv=$*"
exit 0
