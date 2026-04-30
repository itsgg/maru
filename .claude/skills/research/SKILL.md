---
name: research
description: Research a topic before designing or implementing. Combines web search, codebase scan, and prior-art lookup. Produces a short brief the parent agent can use to design a change. Use before non-trivial implementation work or when GENESIS is silent on a question.
argument-hint: <topic>
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob, WebSearch, WebFetch
---

# /research — pre-design brief

The user is about to design or implement something where they want grounded context, not a guess.

## Procedure

1. **Restate.** One sentence on what you're researching and why. If `$ARGUMENTS` is empty, ask the user for the topic.
2. **Codebase scan.** Grep relevant terms in `crates/`, `docs/`, `specs/`, and `GENESIS.md`. Note what already exists.
3. **Prior art in this repo's lineage.** If the topic touches Rust idioms (file locking, shim binaries, env-var redirection, etc.), check `docs/decisions/` for an ADR. If found, treat as binding context.
4. **External research.** Run 2–4 web searches for: official docs, well-known Rust crates that solve the problem, open issues / RFCs on the canonical repos. Skim, don't deep-dive. Cite sources with URLs.
5. **Synthesize.** A tight brief, ≤ 400 words, structured as:
   - **Problem:** one paragraph.
   - **Constraints from GENESIS / ADRs:** bullet list.
   - **Options:** 2–4 named approaches, one paragraph each, with the tradeoff.
   - **Recommendation:** one paragraph naming an option and why.
   - **Open questions:** anything inconclusive that needs the user.
   - **Sources:** linked.

## Boundaries

- **Don't implement.** This skill ends at "here's what I'd do." The parent agent decides whether to act.
- **Don't propose deps not in GENESIS §13** as the recommendation. If a dep outside the budget would be cleaner, name it under "open questions" so the user can decide whether to amend GENESIS.
- **Cite GENESIS sections.** When a constraint comes from GENESIS, give the section number.
