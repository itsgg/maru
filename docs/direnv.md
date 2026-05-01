# Using `maru` with `direnv`

`maru` and [direnv](https://direnv.net/) are complementary. Both let you switch state per-directory; they cover different parts of the problem.

## Resolution chain

The shim resolves the active profile in this order (GENESIS §10):

1. `MARU_PROFILE` env var (non-empty)
2. `.maru` file walked from `cwd` upward
3. `<MARU_HOME>/active.txt`
4. `[defaults].profile` from `state.toml`

`MARU_PROFILE` set by direnv (via `.envrc`) wins over `.maru`. That gives you two equally-supported ways to pin a profile to a directory:

- **`.maru` file** — written by `maru profile pin <name>`. Single line. No shell required. Picked up by the shim regardless of how the harness was launched (terminal, IDE-integrated terminal).
- **direnv `.envrc`** — `export MARU_PROFILE=<name>`. Requires direnv to be installed and the directory to be `direnv allow`'d. Useful if you already use direnv for other env vars in the project.

## Recommendation

If you don't use direnv, use `.maru` — zero dependencies, works everywhere the shim works.

If you already use direnv, prefer `.envrc` for consistency with the rest of your project's env setup. Layer a `.maru` only if you want a backstop for non-direnv shells (e.g., when running `claude` from a tmux session that hasn't loaded direnv yet).

## Example `.envrc`

```sh
export MARU_PROFILE=client-acme

# also useful: pin tool versions, set per-project API keys, etc.
use rust
```

## Pitfalls

- **direnv doesn't run inside IDE extension hosts.** The Anthropic Claude VS Code extension and the Codex VS Code extension don't load `.envrc`. Use `.maru` (which the shim reads regardless of how it was invoked) for those.
- **GUI-launched IDEs don't load shell rc OR `.envrc`.** Same fix: `.maru` if the user launches `claude` from the integrated terminal; otherwise see [`limitations.md`](limitations.md).

## Combining

```sh
# repo-with-pin/.maru
work

# repo-with-pin/.envrc
export MARU_PROFILE=hotfix    # overrides .maru
```

The result: `cd repo-with-pin && claude` activates the `hotfix` profile (env wins). Drop the `.envrc` line and you fall back to `work` (the `.maru` pin).
