---
name: review-branch
description: Review all branch changes against origin/main for correctness and patterns
context: fork
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob
---

Review all commits on the current branch compared to origin/main.

## Process

1. Run `git log --oneline origin/main..HEAD` and `git diff origin/main...HEAD --stat`
2. Run `git diff origin/main...HEAD` to see all changes
3. For non-trivial changes, read full files for context beyond the diff
4. Run `cargo check` for each affected crate
5. Report findings in the format below

## Review Criteria

Evaluate against the standards in `.claude/rules/` and `CLAUDE.md` principles. Additionally for embedded code:

- No heap allocation (`alloc`, `Vec`, `String`) in firmware crates
- All `unsafe` blocks have safety comments justifying correctness
- No blocking operations in async tasks
- Peripheral access follows ownership model
- Embassy task spawning uses correct static lifetimes
- PubSub channel dimensions match actual publisher/subscriber count

## Output Format

```
## Review: <branch-name> (<N> commits)

BLOCKING file.rs:42 — Description of the issue
BLOCKING file.rs:58 — Description of the issue

NIT file.rs:100 — Description of the issue
```

Severity levels:
- `BLOCKING` — Must fix before merge: bugs, security issues, rule violations, build failures
- `NIT` — Optional improvement, take it or leave it

If no issues are found, output: `No issues found.`

## Rules

- Review ALL commits on the branch, not just the latest
- Every finding MUST include a file:line reference
- No praise, no "looks good" summaries, no filler text
- No suggestions without file:line references
- Report only concrete issues found in the actual code
