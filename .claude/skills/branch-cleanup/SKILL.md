---
name: branch-cleanup
description: Prepare branch for merge with full history cleanup
disable-model-invocation: true
allowed-tools: Bash, Read, Edit
---

Rebase toolkit for preparing a branch for merge. Default operation analyzes the
full branch for fixups, redundant commits, and commit message quality, then
presents a cleanup plan for user approval. Rebase operations below are the
tools used to execute the plan.

## Core Technique: GIT_SEQUENCE_EDITOR

All rebase operations use `GIT_SEQUENCE_EDITOR` to avoid interactive editors.
This is the only safe way for an AI agent to perform interactive rebase.

**NEVER use the `reword` action.** It opens an interactive editor which hangs
in non-interactive mode. Always use `edit` + `git commit --amend -m "..."` to
change commit messages.

```bash
# No-op editor for autosquash-only rebases
GIT_SEQUENCE_EDITOR=: git rebase -i --autosquash origin/main

# sed for targeted operations on specific commits
GIT_SEQUENCE_EDITOR="sed -i 's/^pick <HASH>/edit <HASH>/'" git rebase -i origin/main

# Multiple commits in a single rebase
GIT_SEQUENCE_EDITOR="sed -i -e 's/^pick <HASH1>/edit <HASH1>/' -e 's/^pick <HASH2>/edit <HASH2>/'" git rebase -i origin/main

# Bash script for complex todo list rewrites like reordering
GIT_SEQUENCE_EDITOR='bash -c "
  LINE=$(grep \"^pick <HASH>\" \"\$1\")
  sed -i \"/^pick <HASH>/d\" \"\$1\"
  sed -i \"/^pick <TARGET_HASH>/a\\\\$LINE\" \"\$1\"
"' git rebase -i origin/main
```

## Pre-flight (before every rebase)

1. Verify clean working tree: `git status`
2. Create timestamped backup: `git branch backup-$(git branch --show-current)-$(date +%s)`
3. Show current commits: `git log --oneline origin/main..HEAD`

## Absorb: Automatic Fixup Creation

`git absorb` automates fixup commit creation. Given staged changes, it
determines which prior commit each hunk belongs to and creates `fixup!`
commits automatically. This replaces the manual process of identifying
target commits with `git log` and running `git commit --fixup=<hash>`.

The algorithm uses patch commutation to guarantee fixups will never conflict
during autosquash. Hunks that cannot be unambiguously attributed are left
staged with a warning.

### Usage

```bash
# Stage fixes, then absorb into fixup commits
git add <fixed-files>
git absorb --base origin/main

# Preview first without creating commits
git absorb --base origin/main --dry-run

# Create fixups and immediately autosquash them
git absorb --base origin/main --and-rebase
```

**Always pass `--base origin/main`** to search the full branch. The default
search depth is only 10 commits.

### When to use absorb vs manual fixup

| Scenario | Tool |
|----------|------|
| Multiple fixes across files, each attributable to one commit | `git absorb` |
| Fix touches lines not modified by any branch commit | manual `git commit --fixup` or standalone commit |
| New files that have no prior commit to absorb into | manual commit |
| Need to verify target attribution before committing | `git absorb --dry-run`, then manual if unclear |

### Recovery

git-absorb saves `PRE_ABSORB_HEAD` before modifying anything:
```bash
git reset --soft PRE_ABSORB_HEAD
```

## Default Operation: Merge Readiness Cleanup

When invoked without arguments, perform a full analysis of the branch and
present a cleanup plan to the user. Do NOT execute changes until the user
approves the plan.

### Phase 1: Autosquash Fixups

If any `fixup!` or `squash!` commits exist:

```bash
git log --oneline origin/main..HEAD | grep -E 'fixup!|squash!'
```

1. Validate each fixup has a matching target commit on the branch
2. Warn if any fixup is orphaned
3. Run autosquash:
   ```bash
   GIT_SEQUENCE_EDITOR=: git rebase -i --autosquash origin/main
   ```

If no fixups exist, skip to Phase 2.

### Phase 2: Detect Redundant Commits

Find commits that touch the same files and may be squashable.

1. List files changed per commit:
   ```bash
   for hash in $(git rev-list origin/main..HEAD); do
     echo "=== $(git log -1 --oneline $hash) ==="
     git diff-tree --no-commit-id --name-only -r $hash
   done
   ```
2. For files touched by multiple commits, inspect the diffs to determine if:
   - A later commit supersedes an earlier one on the same lines. This is the
     strongest signal for squashing. Example: commit A adds a function, commit
     B rewrites the same function. Commit A has no standalone value.
   - A later commit is a small follow-up fix to an earlier one. Example:
     commit A adds a module, commit B fixes a typo in that module. The fix
     should fold into the original.
   - Two commits modify the same file for genuinely different reasons. These
     should stay separate. Example: commit A adds a feature to module X,
     commit B fixes an unrelated bug in module X.

3. For each pair of potentially redundant commits, record:
   - The two commit hashes and subjects
   - Which files overlap
   - Whether it is a supersede, follow-up fix, or independent change
   - Recommended action: squash, keep separate, or move hunks

### Phase 3: Audit Commit Messages

For every commit on the branch, read the full commit with `git show <hash>`
and check:

1. **Title format**: `module: Imperative verb, capital letter` with max 72 chars
2. **Body explains WHY**: The bullets must explain intent and motivation, not
   enumerate code changes the reviewer can see in the diff
3. **No WHAT bullets**: Flag any bullet that just describes a code change
   without explaining why. Examples of bad bullets:
   - "add X option to module Y" — just restates the diff
   - "update config to use new value" — no motivation given
   - "remove unused import" — fine for a title-only commit, bad as a bullet
     in a multi-line message when it doesn't explain why it was there
4. **Accuracy**: The message must match the actual diff. After fixup folding,
   the diff may have grown beyond what the original message described.

For each commit with issues, record:
- The hash and current message
- What is wrong: missing WHY, inaccurate description, bad format
- Suggested reworded message

### Phase 4: Present Cleanup Plan

Present ALL findings to the user in a structured format:

```
## Fixups
(list of fixups folded, or "none")

## Redundant Commits
(for each pair: hashes, overlap reason, recommended action)

## Commit Message Issues
(for each: hash, problem, suggested fix)

## Proposed Actions
1. Squash X into Y (reason)
2. Reword Z (fix message)
3. ...

## No Changes Needed
(list commits that are already clean)
```

Wait for user approval before proceeding. The user may approve all, reject
some, or modify the plan.

### Phase 5: Execute Approved Changes

Execute the approved plan using the rebase operations documented below.
Order of operations matters:

1. **Squash/drop** first — reduces the number of commits, making subsequent
   operations simpler and less likely to conflict
2. **Move files between commits** — restructure content
3. **Reword** last — messages should reflect final content

Combine as many edits as possible into a single rebase pass by marking
multiple commits with `-e` flags in one `GIT_SEQUENCE_EDITOR` sed command.

### Phase 6: Final Verification

1. Diff against backup must be empty: `git diff backup-<branch>-<ts>..HEAD`
2. `cargo check` for each affected crate
3. Show before/after commit list to user
4. If the diff is not empty, something went wrong. Inform the user and
   do NOT delete the backup.

## Rebase Operations

Reference for all rebase operations used during cleanup execution (Phase 5)
or when the user requests a specific operation.

### Edit a Commit (modify content in place)

Use when: a commit needs its content changed without splitting. This is the
most common rebase operation for fixing review findings, removing lines,
or adjusting code in a specific commit.

1. Mark the commit for editing:
   ```bash
   GIT_SEQUENCE_EDITOR="sed -i 's/^pick <HASH>/edit <HASH>/'" git rebase -i origin/main
   ```
2. The rebase pauses at the target commit. The working tree reflects the
   state as of that commit. Make the changes to the file.
3. Stage and amend:
   ```bash
   git add <modified-files>
   git commit --amend --no-edit
   ```
   Use `--amend -m "new message"` if the message also needs updating.
4. Continue:
   ```bash
   git rebase --continue
   ```

To edit multiple commits in one rebase, mark them all with a single sed
command using `-e` flags. The rebase will pause at each one in order.
After amending each, run `git rebase --continue` to advance to the next.

### Split a Commit

Use when: a commit bundles unrelated changes that belong in separate commits.

1. Mark for editing:
   ```bash
   GIT_SEQUENCE_EDITOR="sed -i 's/^pick <HASH>/edit <HASH>/'" git rebase -i origin/main
   ```
2. Undo the commit but keep changes in working tree:
   ```bash
   git reset HEAD~1
   ```
3. Re-commit in logical groups:
   ```bash
   git add <files-for-group-1> && git commit -m "..."
   git add <files-for-group-2> && git commit -m "..."
   ```
   For partial file splits, use `git add -p <file>` to stage individual hunks.
4. Continue:
   ```bash
   git rebase --continue
   ```

### Reword a Commit

Use when: a commit message is inaccurate or needs updating after fixup folding.

1. Mark for editing:
   ```bash
   GIT_SEQUENCE_EDITOR="sed -i 's/^pick <HASH>/edit <HASH>/'" git rebase -i origin/main
   ```
2. Amend with new message:
   ```bash
   git commit --amend -m "$(cat <<'EOF'
   module: New commit message

   - updated bullet points
   EOF
   )"
   ```
3. Continue:
   ```bash
   git rebase --continue
   ```

### Move Files Between Commits

Use when: specific files belong in a different commit.

1. Mark the source commit for editing:
   ```bash
   GIT_SEQUENCE_EDITOR="sed -i 's/^pick <HASH>/edit <HASH>/'" git rebase -i origin/main
   ```
2. Extract files from the commit:
   ```bash
   git reset HEAD^ -- <file1> <file2>
   git commit --amend --no-edit
   ```
3. Continue rebase: `git rebase --continue`
4. The extracted files are now uncommitted changes. Either:
   - Amend them into a later commit with a second edit rebase, or
   - Create a new commit and reorder it into place

### Reorder Commits

Use when: a commit needs to be at a different position in the branch.

1. Show current order: `git log --oneline origin/main..HEAD`
2. Move a commit after a different one:
   ```bash
   GIT_SEQUENCE_EDITOR='bash -c "
     LINE=$(grep \"^pick <HASH_TO_MOVE>\" \"\$1\")
     sed -i \"/^pick <HASH_TO_MOVE>/d\" \"\$1\"
     sed -i \"/^pick <TARGET_HASH>/a\\\\$LINE\" \"\$1\"
   "' git rebase -i origin/main
   ```
3. Handle any conflicts from the new order.

### Drop a Commit

Use when: a commit should be removed entirely.

```bash
GIT_SEQUENCE_EDITOR="sed -i 's/^pick <HASH>/drop <HASH>/'" git rebase -i origin/main
```

### Full Soft Reset (last resort)

Use when: commits are too interleaved to rebase cleanly. Requires user
confirmation before proceeding.

1. `git reset --soft origin/main`
2. `git reset HEAD -- .`
3. Stage and commit in logical groups using `/commit` skill
4. Verify: `cargo check` for affected crates

## Conflict Handling

When a rebase encounters a conflict:

1. **Investigate first**: read the conflict markers and understand both sides.
   Check what the target branch changed:
   ```bash
   git log -p -n 3 origin/main -- <conflicting-file>
   ```
2. **Simple conflicts** (few files, clear resolution): resolve the files,
   `git add <resolved-files>`, then `git rebase --continue`.
3. **Complex conflicts** (many files, unclear intent): abort immediately
   with `git rebase --abort` and inform the user. Suggest alternatives
   like soft reset or a different rebase strategy.
4. **NEVER escalate** from a failed rebase to `git reset --hard`,
   `git checkout -- .`, or other destructive commands. The only safe
   escape from a stuck rebase is `git rebase --abort`.

## Rules

- NEVER push to remote
- NEVER delete backup branch automatically
- NEVER use destructive commands (`reset --hard`, `checkout -- .`, `clean -f`)
- ALWAYS create backup branch before any rebase
- ALWAYS run `cargo check` for affected crates after rebase completes
- ALWAYS show before/after commit list to user
- ALWAYS abort rebase on unexpected conflicts rather than guessing

## Principles

- Preserve commits by default
- More granular commits are easier to review than large ones
- Analysis is free, action requires approval
- Present the full picture before touching history
- Squash only when a commit has no standalone review value
- Independent changes to the same file are not redundant
- Separate CLAUDE.md changes from code commits
- When in doubt, abort and ask the user
