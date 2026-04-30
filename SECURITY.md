# Security policy

`maru` manages credentials-adjacent state for AI coding agents. We take vulnerabilities seriously even though the project is pre-1.0.

## Supported versions

| Version | Status                            |
| ------- | --------------------------------- |
| 0.x     | Pre-release; security fixes apply |

Once 1.0 ships, we'll commit to fixing the latest minor and one prior.

## Reporting a vulnerability

**Please do not open a public GitHub issue for security reports.**

Email **security@maru.dev** (or the maintainer listed in `Cargo.toml`'s `authors` field if that address isn't yet reachable). Include:

- A clear description of the issue.
- Steps to reproduce, or a proof-of-concept.
- The version and platform you saw it on.
- Any mitigation you've already identified.

If you'd prefer encrypted email, ask in your first message and we'll exchange keys.

## What to expect

- **Acknowledgement within 48 hours.** If you haven't heard back, ping again — assume the message got lost.
- **Triage within 7 days.** We'll confirm the issue, assess severity, and propose a fix timeline.
- **Fix and disclosure within 90 days** for confirmed issues, sooner for severe ones. We'll coordinate disclosure timing with you.

## Scope

In scope:

- Anything that causes `maru` to leak credentials (`.credentials.json`, `auth.json`, `oauth_creds.json`, OAuth tokens, API keys).
- Profile cross-contamination — one profile reading another's state without the user's intent.
- Privilege escalation, arbitrary file write outside `$MARU_HOME`, or arbitrary command execution from a `.maru` file or profile import.
- Anything that bypasses the GENESIS §8 deny-list during clone/export.

Out of scope:

- Bugs in the underlying agent CLIs (Claude Code, Codex, Gemini). Report those upstream.
- Issues that require an attacker to already have your local user account.
- Theoretical issues without a working repro.

## Credit

If you'd like public credit, we'll list you in the `CHANGELOG.md` entry for the fixing release. Anonymous reports are also fine.
