---
name: commit
description: Create atomic git commits following project conventions with chunk-based staging
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob
---

Create atomic commits for staged/unstaged changes using chunk-based staging.

## Commit Format

Title: `module: Verb in imperative style`
- Must match regex: `^[a-z][a-z0-9-]+(: [a-z][a-z0-9-]+)*: [A-Z]`
- Max 72 characters
- NO WIP/TMP
- Imperative verb: Add, Fix, Update, Remove, Refactor
- Module is `rusty-boards` for repo-level, `rusty-boards: <subproject>` for boards
- Module examples: `rusty-boards`, `rusty-boards: temp-sensor`, `rusty-boards: voltage-meter`

Body: Bullet points only
- Lines must be blank, start with `- `, or be indented continuation
- Max 120 characters per line
- NO prose paragraphs
- NO Co-Authored-By
- NO Claude signatures
- NO emojis

## Process

1. Run `git status` and `git diff` to understand all changes
2. Run `git log -5 --oneline` to see recent commit style
3. Identify logical groups of changes that belong together
4. For each logical group:
   - Stage specific chunks with `git add -p <file>` for modified files
   - For new files: `git add -N <file> && git add -p <file>`
   - Verify staged changes: `git diff --cached`
   - Run `cargo check` in the affected firmware crate directory
   - Create commit using HEREDOC:
     ```bash
     git commit -m "$(cat <<'EOF'
     module: Title here

     - bullet point describing what changed
     - another bullet if needed
     EOF
     )"
     ```
5. Run `git log --oneline -5` to verify

## Fixup Commits

For iterations after review feedback, use fixup commits:
```bash
git commit --fixup=HEAD
```

Or target a specific commit:
```bash
git commit --fixup=<commit-hash>
```

Fixups will be squashed later with `/branch-cleanup`.

## Chunk Staging Reference

Interactive patch mode (`git add -p`) commands:
- `y` - stage this hunk
- `n` - skip this hunk
- `s` - split into smaller hunks
- `q` - quit, do not stage remaining hunks

## Rules

- One logical change per commit
- Separate unrelated changes into different commits
- Lock files (`Cargo.lock`, `flake.lock`) get their own commit, never bundled with source
- AI/tooling config (`.claude/`, `CLAUDE.md`) gets its own commit, never bundled with code
- NEVER push to remote
- NEVER use --amend unless explicitly requested
- NEVER skip cargo check
