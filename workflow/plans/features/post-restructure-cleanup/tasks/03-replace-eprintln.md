## Task: Replace eprintln! with tracing::error! in headless mode

**Objective**: Replace 3 `eprintln!` calls in `HeadlessEvent::emit()` with `tracing::error!()` to comply with the project's logging standard ("NEVER use println! or eprintln!").

**Review Issue**: #3 (MAJOR) - eprintln! usage in HeadlessEvent::emit() error paths

**Depends on**: None

### Scope

- `src/headless/mod.rs`: Replace 3 `eprintln!` calls at lines 115, 123, 129

### Details

#### Current Code (src/headless/mod.rs)

Three error fallback paths in `HeadlessEvent::emit()` use `eprintln!`:

**Line 115** - Serialization failure:
```rust
Err(e) => {
    // Fallback error - should never happen with our types
    eprintln!("Failed to serialize event: {}", e);
    return;
}
```

**Line 123** - Stdout write failure:
```rust
if let Err(e) = writeln!(stdout, "{}", json) {
    eprintln!("Failed to write to stdout: {}", e);
    return;
}
```

**Line 129** - Stdout flush failure:
```rust
if let Err(e) = stdout.flush() {
    eprintln!("Failed to flush stdout: {}", e);
}
```

#### Fix

Replace each `eprintln!` with `tracing::error!`:

```rust
Err(e) => {
    tracing::error!("Failed to serialize headless event: {}", e);
    return;
}
```

```rust
if let Err(e) = writeln!(stdout, "{}", json) {
    tracing::error!("Failed to write headless event to stdout: {}", e);
    return;
}
```

```rust
if let Err(e) = stdout.flush() {
    tracing::error!("Failed to flush headless stdout: {}", e);
}
```

Ensure `tracing` is imported. The binary crate already depends on `tracing` via `fdemon-core::prelude::*`, but verify the import is available in `src/headless/mod.rs`. If not, add `use tracing::error;` at the top.

### Acceptance Criteria

1. Zero `eprintln!` calls remain in `src/headless/mod.rs`
2. All 3 error paths use `tracing::error!()` instead
3. `cargo check` passes
4. `cargo clippy -- -D warnings` passes
5. Verify no other `eprintln!` calls exist anywhere in the codebase (grep for `eprintln!` and `println!`)

### Testing

No new tests needed -- these are error paths that "should never happen" (per the existing comment). The change is purely a logging infrastructure swap with identical behavior.

Run a quick grep to verify no other violations:
```bash
cargo clippy --workspace --lib -- -D warnings
grep -rn 'eprintln!\|println!' src/ crates/ --include='*.rs' | grep -v '#\[cfg(test)\]' | grep -v 'mod tests'
```

### Notes

- This is a quick, isolated fix (~5 minutes)
- The `tracing` subscriber in headless mode writes to a log file (stdout is reserved for NDJSON), so these errors will go to the log file rather than interfering with structured output
- Consider removing the "should never happen" comment on line 114 -- it adds no value

---

## Completion Summary

**Status:** Not Started
