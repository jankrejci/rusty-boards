---
name: fix-review
description: Apply fixes from review findings with fixup commits
disable-model-invocation: true
allowed-tools: Bash, Read, Edit, Write, Grep, Glob
---

Apply fixes from `/review-branch` findings passed in `$ARGUMENTS`.

## Process

1. Parse findings from arguments (BLOCKING and NIT items with file:line references)
2. Address BLOCKING issues first
3. For each issue:
   - Read the file and understand the problem
   - Apply the minimal fix
   - Run `cargo check` for the affected crate
   - Identify the commit that introduced the issue: `git log --oneline origin/main..HEAD -- <file>`
   - Create a fixup commit targeting that commit: `git commit --fixup=<hash>`
   - Show the user what was fixed and stop before moving to the next issue
4. Only address NIT items after all BLOCKING issues are resolved and user confirms

## Rules

- One logical fix per fixup commit
- Never batch unrelated fixes into a single commit
- Never auto-fix NITs without user confirmation
- Follow commit rules from `.claude/rules/git.md`
- NEVER push to remote
- NEVER skip `cargo check`
