# Rust Development Rules

Code style rules for all Rust code in this repository.

## Control Flow

Prefer flat code over nested code. Maximum two levels of nesting in any function.

### Early returns

Use guard clauses and early returns to keep the happy path at the top level.

```rust
// Good: guard clause with let-else.
let Some(value) = optional else { return; };

// Good: early return on error.
let Ok(data) = try_parse(input) else {
    log::warn!("bad input");
    return;
};

// Bad: nested if-let.
if let Some(value) = optional {
    if let Ok(data) = try_parse(value) {
        // deeply nested happy path
    }
}
```

### Avoid else branches

Prefer early return, `match`, or `unwrap_or_else` over if/else.

```rust
// Good
let cfg = Config::load(&path).unwrap_or_else(|e| {
    log::warn!("load failed: {}", e);
    Config::default()
});

// Bad
let cfg = match Config::load(&path) {
    Ok(c) => c,
    Err(e) => {
        log::warn!("load failed: {}", e);
        Config::default()
    }
};
```

### Prefer match over if-chains

When checking multiple string conditions or enum variants, use `match` instead of chained `if`/`else if`.

### Extract functions to reduce nesting

When a loop body contains another loop or complex logic, extract the inner part into a named function.

## Functions

### Return values instead of output parameters

Functions should return their results. Do not pass `&mut Vec<T>` as an output parameter. Use `.extend()` at the call site if accumulating results.

```rust
// Good
fn parse(input: &str) -> Vec<Metric> { ... }
lines.extend(parse(input));

// Bad
fn parse(input: &str, output: &mut Vec<Metric>) { ... }
```

### Import common types

Import frequently used types to reduce noise:

```rust
use anyhow::Result;  // not anyhow::Result<T> everywhere
```

## Naming

### No single-letter variables

Use descriptive names: `index` not `i`, `key` not `k`, `value` not `v`. Exception: conventional iterator variables in very short closures.

### Named constants for magic numbers

Every literal number or string that represents a tunable value must be a named `const` with `SCREAMING_SNAKE_CASE`.

```rust
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(10);
```

## String Processing

Prefer string methods over character-by-character iteration with `.chars().map()`. Use `.filter_map()` instead of `.map()` followed by `.filter()`.

## Testing

Keep tests in `src/tests/` using the `#[path]` attribute. This keeps source files
small while preserving private function access:

```rust
// src/foo.rs - after imports
#[cfg(test)]
#[path = "tests/foo.rs"]
mod tests;
```

```rust
// src/tests/foo.rs - no wrapper, no #[cfg(test)]
use super::*;

#[test]
fn it_works() { ... }
```

Note: `include_str!` paths in test files resolve relative to the test file location.

## Standard Patterns

Use standard library and ecosystem abstractions. Do not reimplement what a well-known crate provides.

## Lints

Enable pedantic clippy lints in every crate:

```toml
[lints.clippy]
pedantic = "warn"
unwrap_used = "deny"
```
