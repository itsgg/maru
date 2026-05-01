<#
.SYNOPSIS
    Installs maru (CLI + shim) on Windows.
.DESCRIPTION
    Fetches the latest release and installs both the `maru` and
    `maru-shim` binaries via the per-package PowerShell installers
    that `dist` produces, then runs `maru install` to wire the
    per-harness shims (claude/codex/gemini) into $MARU_HOME\bin.
.EXAMPLE
    iwr https://raw.githubusercontent.com/itsgg/maru/main/scripts/install.ps1 | iex
.EXAMPLE
    & ([scriptblock]::Create((iwr https://raw.githubusercontent.com/itsgg/maru/main/scripts/install.ps1).Content)) -NoShellRc
#>

[CmdletBinding()]
param(
    [switch]$NoShellRc
)

$ErrorActionPreference = 'Stop'

$repo = 'itsgg/maru'
$apiUrl = "https://api.github.com/repos/$repo/releases"

function Write-Info($msg) {
    Write-Host "maru-installer: $msg"
}

# Find the latest release tag — including prereleases. The
# `/releases/latest` endpoint excludes prereleases and returns 404
# while we're alpha; the list endpoint returns all releases newest first.
function Get-LatestTag {
    $headers = @{ 'Accept' = 'application/vnd.github+json' }
    $releases = Invoke-RestMethod -Uri "$apiUrl?per_page=1" -Headers $headers -UseBasicParsing
    if ($releases.Count -eq 0) {
        throw "no releases found at $apiUrl"
    }
    return $releases[0].tag_name
}

$tag = Get-LatestTag
Write-Info "latest release: $tag"
$base = "https://github.com/$repo/releases/download/$tag"

function Invoke-PerBinaryInstaller($label, $url) {
    Write-Info "running $label installer"
    $script = Invoke-WebRequest -Uri $url -UseBasicParsing
    Invoke-Expression $script.Content
}

Invoke-PerBinaryInstaller 'maru-cli' "$base/maru-cli-installer.ps1"
Invoke-PerBinaryInstaller 'maru-shim' "$base/maru-shim-installer.ps1"

# Both installers drop binaries into $CARGO_HOME\bin (defaults to
# $env:USERPROFILE\.cargo\bin); make sure that's on PATH for this
# session so the `maru install` invocation below resolves.
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$cargoBin = Join-Path $cargoHome 'bin'
$env:Path = "$cargoBin;$env:Path"

if (-not (Get-Command maru -ErrorAction SilentlyContinue)) {
    Write-Error "maru did not land on PATH after install. Looked under $cargoBin. Add it to your PATH and re-run: maru install"
    exit 1
}

Write-Info "running 'maru install' to wire shim symlinks"
if ($NoShellRc) {
    & maru install --no-shell-rc
} else {
    & maru install
}

Write-Info "done. Open a new terminal so the maru shim dir takes effect on PATH."
