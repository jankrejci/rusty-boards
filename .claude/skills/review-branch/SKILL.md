---
name: review-branch
description: Review all branch changes against origin/main for correctness and patterns
allowed-tools: Bash, Read, Grep, Glob, Task, WebSearch, WebFetch
---

Review all commits on the current branch compared to origin/main.

## Process

1. **Get branch overview**: Run `git log --oneline origin/main..HEAD` and `git diff origin/main...HEAD --stat`
2. **Read the full diff**: Run `git diff origin/main...HEAD` to see all changes
3. **Read modified files**: For non-trivial changes, read full files for context
4. **Verify builds**: Run `cargo check` for each affected firmware crate
5. **Report findings**: Group by file or feature with file:line references

## Review Checklist

**General:**
- [ ] Code does what commit messages claim
- [ ] No obvious bugs or logic errors
- [ ] Error handling is appropriate
- [ ] No security issues introduced
- [ ] No hardcoded values that should be configurable
- [ ] No stale references after renames

**Embedded Rust specific:**
- [ ] No heap allocation (no `alloc`, no `Vec`, no `String`)
- [ ] Proper `unsafe` usage with clear safety comments
- [ ] No blocking operations in async tasks
- [ ] Peripheral access follows ownership model
- [ ] Embassy task spawning uses correct static lifetimes
- [ ] Metrics follow Prometheus text format convention
- [ ] PubSub channel dimensions match actual publisher/subscriber count
- [ ] Watchdog timeout is appropriate for task duration

**Firmware patterns:**
- [ ] Follows voltage-meter reference architecture
- [ ] Config constants in config.rs, not scattered across modules
- [ ] Proper defmt logging levels (trace for periodic, info for startup)
- [ ] probe-rs runner configured correctly in .cargo/config.toml
- [ ] `cargo check` passes for affected crates

**Branch hygiene:**
- [ ] Commits are logically grouped
- [ ] No unrelated changes mixed together
- [ ] Fixup commits reference correct targets

## Reporting Format

```
## Branch Review: <branch-name>

### Commits
- List of commits reviewed

### Issues
- file.rs:42 - Issue description
- file.rs:58 - Another issue

### Suggestions
- Optional improvement idea

### Verified
- cargo check passes
```

## Rules

- Review ALL commits on the branch, not just the latest
- Reference specific lines when reporting issues
- Distinguish blocking issues from suggestions
- Run cargo check for any firmware changes
- Use `Task` agents for heavy exploration to save context
