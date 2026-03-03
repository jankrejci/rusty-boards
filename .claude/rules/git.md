# Git Rules

## Commit Format

```
module: Title in imperative style

- lowercase bullet describing implementation detail
- another bullet if needed
```

- Title: capital letter after colon, imperative verb
- Body: bullet points only, lowercase start, no prose paragraphs
- NO Co-Authored-By, NO Claude signatures, NO emojis
- Module: `rusty-boards` for repo-level, `rusty-boards: <board>` for firmware
- Examples: `rusty-boards`, `rusty-boards: temp-sensor`, `rusty-boards: voltage-meter`

## Commit Discipline

Each commit must:
- Pass `cargo check` for the affected crate
- Be a single logical change reviewable in isolation
- Build progressively toward the goal

Separate commits required for:
- Lock files (`Cargo.lock`, `flake.lock`) — never bundled with source changes
- AI/tooling config (`.claude/`, `CLAUDE.md`, `AGENTS.md`) — never bundled with code changes

Prefer many small commits over few large ones during development.

## Branch Cleanup

Before merge, consolidate into clean logical commits:

1. `git branch backup-<branch>`
2. `git reset --soft origin/main`
3. `git reset HEAD -- .`
4. Commit in logical groups by file/feature
5. `cargo check` for each affected crate

Principles:
- One logical change per commit, keep commits small
- Squash duplicate/related changes
- Drop immediately superseded commits
- Separate CLAUDE.md/AGENTS.md changes from code commits
- Soft reset is cleaner than rebase when commits are interleaved
