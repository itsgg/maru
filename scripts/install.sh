#!/usr/bin/env sh
# maru installer — fetches the latest release and installs both the
# `maru` and `maru-shim` binaries into $CARGO_HOME/bin (defaults to
# ~/.cargo/bin), then runs `maru install` to wire the per-harness
# symlinks (claude/codex/gemini) into $MARU_HOME/bin.
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/itsgg/maru/main/scripts/install.sh | sh
#
# Or, to skip the shell rc edit (you'll add $MARU_HOME/bin to PATH yourself):
#   curl -sSL https://raw.githubusercontent.com/itsgg/maru/main/scripts/install.sh | sh -s -- --no-shell-rc
#
# This script is a thin wrapper over the per-binary installer scripts that
# `dist` produces on each release. We wrap them so the user sees a single
# install command instead of having to run two separate ones.

set -eu

REPO="itsgg/maru"
API_URL="https://api.github.com/repos/${REPO}/releases"

NO_SHELL_RC=""
for arg in "$@"; do
    case "$arg" in
        --no-shell-rc) NO_SHELL_RC="--no-shell-rc" ;;
        *) printf 'maru-installer: unknown argument: %s\n' "$arg" >&2; exit 64 ;;
    esac
done

log() { printf 'maru-installer: %s\n' "$*"; }

require() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'maru-installer: required command not found on PATH: %s\n' "$1" >&2
        exit 1
    fi
}

require curl
require sh

# Find the latest release tag — including prereleases. The
# `/releases/latest` endpoint excludes prereleases and returns 404
# while we're alpha; the list endpoint returns all releases newest
# first.
find_latest_tag() {
    curl -sSfL -H "Accept: application/vnd.github+json" "${API_URL}?per_page=1" \
        | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' \
        | head -n 1
}

TAG="$(find_latest_tag)"
if [ -z "$TAG" ]; then
    printf 'maru-installer: could not determine latest release tag from %s\n' "$API_URL" >&2
    exit 1
fi
log "latest release: ${TAG}"

DOWNLOAD_BASE="https://github.com/${REPO}/releases/download/${TAG}"

run_installer() {
    label="$1"
    url="$2"
    log "running ${label} installer"
    # shellcheck disable=SC2086
    curl -sSfL "$url" | sh
}

run_installer "maru-cli" "${DOWNLOAD_BASE}/maru-cli-installer.sh"
run_installer "maru-shim" "${DOWNLOAD_BASE}/maru-shim-installer.sh"

# Both installers drop binaries into $CARGO_HOME/bin (defaults to
# $HOME/.cargo/bin); make sure that's on PATH for this shell so the
# `maru install` invocation below resolves.
CARGO_BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
export PATH="${CARGO_BIN}:${PATH}"

if ! command -v maru >/dev/null 2>&1; then
    log "ERROR: maru did not land on PATH after install. Looked under ${CARGO_BIN}."
    log "Add it to your shell rc and re-run: maru install"
    exit 1
fi

log "running 'maru install' to wire shim symlinks"
# shellcheck disable=SC2086
maru install ${NO_SHELL_RC}

log "done. Open a new terminal (or source your shell rc) so the maru shim dir takes effect on PATH."
