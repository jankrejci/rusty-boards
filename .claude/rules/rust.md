---
paths: **/*.rs
---

# Rust Rules

Rules for all Rust code in this repository. Applies to firmware, tools, and
libraries equally.

## Error Discipline

Enforced by clippy lints in each crate's `Cargo.toml`:

```toml
[lints.clippy]
unwrap_used = "deny"
```

When adding a new crate, include this lint configuration.

### `unwrap()` — banned

Never use `unwrap()`. No exceptions. Clippy denies it.

### `expect()` — allowed with justification

Every `expect()` call requires a comment on the line above stating the invariant
that guarantees it cannot fail. The expect message itself uses the `BUG:` prefix
to signal a violated invariant.

Prefer `?` propagation wherever the function returns `Result` or `Option`. Use
`expect()` only in `main()` or top-level setup where `?` is not available.

```rust
// Channel has capacity for 4 publishers, only 2 are created.
METRICS_CHANNEL
    .publisher()
    .expect("BUG: not enough publishers")
```

## Verification After Changes

After modifying any `.rs` file, run both checks before considering the change
complete:

1. `cargo clippy` for the affected crate — all warnings are errors
2. `cargo fmt --check` for the affected crate — must pass clean

If the crate requires a nix dev shell (check for `flake.nix` in the crate's
parent directories), run checks inside `nix develop --command`.

If clippy warns on upstream macro expansions that cannot be fixed in user code,
add a scoped `#[allow(clippy::lint_name)]` on the call site with a comment
naming the upstream source.
