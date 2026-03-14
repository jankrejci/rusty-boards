---
name: fix-review
description: Apply fixes from review findings with conflict-safe fixup commits
disable-model-invocation: true
allowed-tools: Bash, Read, Edit, Write, Grep, Glob
---

Apply fixes from `/review-branch` findings passed in `$ARGUMENTS`.

## Process

1. Parse findings from arguments (BLOCKING and NIT items with file:line references)
2. Classify each finding: code fix, structural fix, or skip (with rationale)
3. Present the full list to user with proposed action for each item
4. Wait for user approval
5. Apply all approved fixes (BLOCKING and NIT together)

### Fix classification

**Code fixes** (apply via fixup commit): the finding requires changing file
content but the commit structure is correct. Examples: remove duplicate line,
fix typo, add missing check, change a value.

**Structural fixes** (apply via rebase edit): the finding requires changing
commit boundaries. Examples: split a commit, move files between commits,
remove a file from the wrong commit. Use the patterns from `/branch-cleanup`.

### For each code fix:

#### Step 1: Identify the target commit

Find the commit that introduced the issue:
```bash
git log --oneline origin/main..HEAD -- <file>
```

#### Step 2: Conflict prevention check

Before creating a fixup, verify the fix will not conflict during autosquash:

1. Read the target commit's diff for the file: `git show <hash> -- <file>`
2. Verify the issue exists in lines modified by the target commit
3. Check if later commits also modified the same lines:
   ```bash
   git log --oneline <hash>..HEAD -- <file>
   ```
   If later commits touched the same lines, fixup the **latest** commit
   that modified those lines instead of the original
4. If no commit cleanly owns the lines, create a standalone commit instead

#### Step 3: Apply minimal fix

- Read the file and understand the problem
- Apply the minimal fix for this specific finding
- Do NOT introduce new changes unrelated to the finding
- Stage only the fixed file

#### Step 4: Verify staged changes

1. Run `git diff --cached` and confirm the staged changes only touch
   sections relevant to the finding
2. Run `cargo check` for the affected crate

#### Step 5: Create fixup commit

**Multiple code fixes**: if several fixes are ready and each touches lines
clearly owned by a single prior commit, apply all fixes, stage them, and use
`git absorb --base origin/main` to create all fixup commits at once. Use
`--dry-run` first to verify attribution. This replaces steps 1-4 per fix.

**Single fix or ambiguous attribution**: create the fixup manually:
```bash
git commit --fixup=<target-hash>
```

If conflict was unavoidable in step 2, create a standalone commit instead:
```bash
git commit -m "module: Fix description"
```

### For each structural fix:

#### Step 1: Create backup

```bash
git branch backup-$(git branch --show-current)-$(date +%s)
```

#### Step 2: Apply the rebase operation

Use the appropriate pattern from `/branch-cleanup`:

**Split a commit:**
```bash
GIT_SEQUENCE_EDITOR="sed -i 's/^pick <HASH>/edit <HASH>/'" git rebase -i origin/main
git reset HEAD~1
git add <files-for-group-1> && git commit -m "..."
git add <files-for-group-2> && git commit -m "..."
git rebase --continue
```

**Edit a commit in place** (modify content of a specific commit):
```bash
GIT_SEQUENCE_EDITOR="sed -i 's/^pick <HASH>/edit <HASH>/'" git rebase -i origin/main
# make changes to the file
git add <modified-files>
git commit --amend --no-edit
git rebase --continue
```

**Reword a commit message** (fix inaccurate or incomplete message):
```bash
GIT_SEQUENCE_EDITOR="sed -i 's/^pick <HASH>/edit <HASH>/'" git rebase -i origin/main
git commit --amend -m "$(cat <<'EOF'
module: Updated commit message

- corrected or added bullets
EOF
)"
git rebase --continue
```

NEVER use the `reword` action. It opens an interactive editor which hangs in
non-interactive mode. Always use `edit` + `git commit --amend -m "..."`.

**Multiple operations**: mark all target commits for edit in a single sed
command with `-e` flags. The rebase pauses at each in order.

#### Step 3: Verify

1. Run `cargo check` for affected crates
2. If a backup exists from before the structural fix, verify
   `git diff backup-<branch>-<ts>..HEAD` shows only the intended changes

### After all fixes are applied

Show a summary of what was done:
```
## Fixes Applied

fixup! <target-msg> -- fixed <description>
rebase-edit <target-msg> -- <description>
standalone: <msg> -- <description> (conflict avoidance)
skipped: <description> -- <rationale>
```

## Rules

- One logical fix per fixup commit
- Never batch unrelated fixes into a single commit
- Never introduce changes beyond what the finding requires
- Follow commit rules from `CLAUDE.md`
- NEVER push to remote
- NEVER skip `cargo check` when Rust code changed
- If a rebase conflicts, abort with `git rebase --abort` and inform the user
