---
name: next
description: Recommend the next concrete task to pick up. Looks at git state, TODO.md if present, current GENESIS phase, and any halt notes. Use at start of session or whenever the user asks "what's next?".
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob
---

# /next — what to work on next

Single-shot recommendation. Don't start the work; just suggest it.

## Procedure

1. **Git state.** `git status --short`, `git log --oneline -5`, `git tag --list 'phase-*-complete' --sort=-creatordate | head -1`.
2. **Phase.** Current phase = (last tagged phase) + 1, or Phase 0 if no tags. Read the phase's checklist in GENESIS §14.
3. **TODO.md.** If `TODO.md` exists, read it and identify the next unchecked item respecting any dependencies (`depends-on:` markers).
4. **Halt notes.** `ls docs/notes/autopilot-halt-*.md 2>/dev/null | head -1`. If one exists and is more recent than the latest commit, read it — that's where work stopped.
5. **Open ADRs / specs.** `ls specs/*.md docs/decisions/*.md` — anything with `Status: draft` or `Status: proposed` is in-flight.

## Output

Five lines max:

```
Phase:    <N> — <phase title from GENESIS>
Branch:   <name>  (<dirty count> dirty)
Last:     <last commit short>
Halt:     <one-line halt reason, or "none">
Next:     <one specific actionable task with task ID if from TODO.md>
```

Then a single follow-up line: "Want me to /autopilot the next task, or open it as an issue first?"

Do not start the work.
