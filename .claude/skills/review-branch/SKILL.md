---
name: review-branch
description: Deep review of all branch changes against origin/main — code, commits, and CI readiness
context: fork
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob
---

Deep, exhaustive review of all commits on the current branch compared to origin/main. The goal is that after all findings are fixed, the branch is merge-ready and all pipeline checks will pass. Do not leave anything for a second pass — find everything in one review.

## Process

### Phase 1: Gather context

1. `git log --oneline origin/main..HEAD` — list all commits
2. `git diff origin/main...HEAD --stat` — see which files changed
3. `git diff origin/main...HEAD` — full diff of all changes
4. For non-trivial changes, read full files for context beyond the diff

### Phase 2: Run all checks

Run every check that CI would run. Report failures as BLOCKING findings.

For each crate with changes:
- `cargo check` — compilation
- `cargo clippy -- -D warnings` — lint (treat warnings as errors)
- `cargo fmt --check` — formatting

For each board/tool with a Nix flake:
- `nix flake check` — runs all configured checks (clippy, fmt, DRC, ERC)

If a crate cannot be checked (e.g. missing Xtensa toolchain), note this explicitly rather than silently skipping it.

### Phase 3: Review commit messages

For each commit, verify:
- Follows format from `.claude/rules/git.md` (module prefix, imperative title, bullet body)
- Every claim in the commit body (dependency versions, API names, what changed) matches the actual diff
- One logical change per commit
- Lock files not bundled with source changes
- AI/tooling config not bundled with code changes
- No Co-Authored-By, no AI signatures, no emojis

### Phase 4: Review code

Evaluate against `.claude/rules/` and `CLAUDE.md` principles.

General:
- Correctness: logic errors, off-by-one, race conditions, missing error handling
- Security: injection, unsafe without justification, secrets in code
- Consistency: follows existing patterns in the codebase
- Dead code: unused imports, unreachable branches, commented-out code
- Duplication: same content defined in multiple places (single source of truth)

Embedded firmware (when applicable):
- No heap allocation (`alloc`, `Vec`, `String`, `Box`)
- No `unwrap()`, `expect()`, `panic!()` outside tests
- No blocking operations in async tasks
- Peripheral access follows ownership model (move semantics)
- Embassy task spawning uses correct static lifetimes
- PubSub channel dimensions match actual publisher/subscriber count
- Integer arithmetic for MCUs without FPU
- Cooperative watchdog implementation

### Phase 5: Cross-cutting concerns

- Are new files/modules properly integrated (imports, mod declarations)?
- Do Nix flakes reference correct paths after any restructuring?
- Are `.cargo/config.toml` runner and target settings correct?
- Do `rust-toolchain.toml` channels match the target architecture?

## Output Format

```
## Review: <branch-name> (<N> commits)

### Check Results

cargo check <crate>: PASS/FAIL
cargo clippy <crate>: PASS/FAIL
cargo fmt --check <crate>: PASS/FAIL
nix flake check <path>: PASS/FAIL/SKIPPED (reason)

### Findings

BLOCKING file.rs:42 -- Description of the issue
BLOCKING file.rs:58 -- Description of the issue

NIT file.rs:100 -- Description of the issue
```

Severity levels:
- `BLOCKING` — Must fix before merge: bugs, build failures, lint errors, format violations, rule violations, incorrect commit messages
- `NIT` — Optional improvement, style preference, take it or leave it

If no issues are found, output: `No issues found. Branch is merge-ready.`

## Rules

- Review ALL commits on the branch, not just the latest
- Run ALL checks — do not skip any. If a check cannot run, report why.
- Every finding MUST include a file:line reference (or commit hash for commit message issues)
- Findings must be exhaustive: if this review passes, the branch is ready to merge
- No praise, no "looks good" summaries, no filler text
- No suggestions without file:line references
- Report only concrete issues found in the actual code or checks
- Do not invent issues that are not evidenced by code, diffs, or check output
