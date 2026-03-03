---
name: branch-cleanup
description: Consolidate branch with autosquash, targeted rebase operations, or full soft reset
disable-model-invocation: true
allowed-tools: Bash, Read
---

Consolidate branch commits before merge. Pick the right operation for the task.

## When to Use

- Before merging a feature branch to main
- After iterative development with fixup commits
- When a commit bundles unrelated changes and needs splitting
- When commit order needs fixing
- When specific files belong in a different commit

## Rules

- NEVER force push without user confirmation
- NEVER delete backup branch automatically
- ALWAYS verify cargo check passes after rebase
- ALWAYS show before/after commit list to user

## Step 0: Always Do First

1. Verify clean working tree: `git status`
2. Create backup branch: `git branch backup-$(git branch --show-current)-$(date +%s)`
3. Show current commits: `git log --oneline origin/main..HEAD`
4. Identify which operation is needed and confirm with user

## Operation: Autosquash Fixups

Use when: branch has `fixup!` or `squash!` commits to fold in.

```bash
GIT_SEQUENCE_EDITOR=: git rebase -i --autosquash origin/main
```

## Operation: Split a Commit

Use when: a commit bundles unrelated changes that should be separate commits.

1. Identify the commit to split: `git log --oneline origin/main..HEAD`
2. Mark it for editing:
   ```bash
   GIT_SEQUENCE_EDITOR="sed -i 's/^pick <SHORT_HASH>/edit <SHORT_HASH>/'" git rebase -i origin/main
   ```
3. At the paused commit, undo it but keep changes staged:
   ```bash
   git reset --soft HEAD^
   ```
4. Unstage everything:
   ```bash
   git reset HEAD -- .
   ```
5. Re-commit in logical groups:
   ```bash
   git add <files-for-group-1> && git commit -m "..."
   git add <files-for-group-2> && git commit -m "..."
   ```
6. Continue rebase:
   ```bash
   git rebase --continue
   ```

## Operation: Reorder Commits

Use when: a commit needs to move to a different position in history.

1. Show current order: `git log --oneline origin/main..HEAD`
2. Use a script as GIT_SEQUENCE_EDITOR to rewrite the todo list:
   ```bash
   GIT_SEQUENCE_EDITOR='bash -c "
     LINE=$(grep \"^pick <HASH_TO_MOVE>\" \"\$1\")
     sed -i \"/^pick <HASH_TO_MOVE>/d\" \"\$1\"
     sed -i \"/^pick <HASH_AFTER>/a\\\\$LINE\" \"\$1\"
   "' git rebase -i origin/main
   ```
   Replace `<HASH_TO_MOVE>` with the commit to relocate and `<HASH_AFTER>` with the commit it should follow.
3. Resolve any conflicts that arise from the new order.

## Operation: Move Files Between Commits

Use when: specific files in one commit belong in an adjacent commit.

1. Mark the source commit (the one with the misplaced files) for editing:
   ```bash
   GIT_SEQUENCE_EDITOR="sed -i 's/^pick <SHORT_HASH>/edit <SHORT_HASH>/'" git rebase -i origin/main
   ```
2. At the paused commit, extract the files:
   ```bash
   git reset HEAD^ -- <file1> <file2>
   git commit --amend --no-edit
   ```
   The files are now unstaged changes in the working tree.
3. Continue rebase: `git rebase --continue`
4. When rebase finishes, the extracted files are uncommitted changes. Amend them into the correct commit using a second rebase, or commit them as a new commit and reorder.

Alternative for moving files to the *next* commit:
1. Mark the source commit for editing (same as above).
2. Extract files and amend (same as above).
3. Continue rebase — the next commit replays on top. If the files were added by the source commit, the next commit may conflict. Resolve by accepting the working tree version.

## Operation: Drop a Commit

Use when: a commit is entirely superseded or unwanted.

```bash
GIT_SEQUENCE_EDITOR="sed -i 's/^pick <SHORT_HASH>/drop <SHORT_HASH>/'" git rebase -i origin/main
```

## Operation: Full Soft Reset (Last Resort)

Use when: rebase cannot produce clean commits due to complex interleaved changes. ASK USER for approval before proceeding.

1. `git reset --soft origin/main`
2. `git reset HEAD -- .`
3. Stage and commit in logical groups using `/commit` skill
4. Verify: `cargo check` for affected crates

Soft reset rewrites history more aggressively than rebase. Only use when targeted operations cannot produce clean commits.

## Final Step: Always Verify

1. Compare with backup: `git diff backup-<branch>-<timestamp>..HEAD` must be empty
2. `cargo check` for each affected crate
3. Show final commits: `git log --oneline origin/main..HEAD`
4. Show user the before/after comparison

## Principles

- One logical change per final commit
- Squash all fixups into their target commits
- Drop commits that are immediately superseded
- Separate CLAUDE.md/AGENTS.md changes from code commits
- Separate lock file changes from source changes
- Preserve struggle documentation in code comments, not commit history
