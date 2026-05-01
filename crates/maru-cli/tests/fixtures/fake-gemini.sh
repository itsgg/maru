#!/usr/bin/env bash
# Fake `gemini` binary for e2e tests. Prints the env vars maru should
# have set, then exits 0.
set -u
echo "FAKE_GEMINI_CALLED"
echo "GEMINI_CLI_HOME=${GEMINI_CLI_HOME:-<unset>}"
echo "argv=$*"
exit 0
