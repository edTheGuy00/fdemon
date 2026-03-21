## Task: Critical Fixes in handlers.rs

**Objective**: Fix the production `.expect()` panic, the column=0 underflow in completions, and the silent `unwrap_or_default` in attach — three blocking issues all in `handlers.rs`.

**Depends on**: None

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Fix three issues (H1, M5, M7)

**Files Read (Dependencies):**
- `crates/fdemon-dap/src/adapter/breakpoints.rs`: Understand `add_with_condition` / `lookup_by_dap_id` contract
- `crates/fdemon-dap/src/adapter/tests/`: Existing test patterns

### Details

#### Fix 1: H1 — Replace `.expect()` at line 343

The only production `.expect()` in the adapter is at `handlers.rs:343`:

```rust
let entry = self
    .breakpoint_state
    .lookup_by_dap_id(dap_id)
    .expect("entry was just inserted");
```

Inside `handle_set_breakpoints`, `add_with_condition()` returns a `dap_id` that was just inserted into `by_dap_id`. The invariant is structurally sound (single-threaded `&mut self`), but `.expect()` in production violates CODE_STANDARDS.md. Replace with:

```rust
let entry = match self.breakpoint_state.lookup_by_dap_id(dap_id) {
    Some(e) => e,
    None => {
        // Invariant: entry was just inserted by add_with_condition.
        // If this is ever reached, breakpoint_state has a bug.
        tracing::error!("Breakpoint state inconsistency: dap_id {} not found after insert", dap_id);
        continue; // Skip this breakpoint, don't crash the session
    }
};
```

#### Fix 2: M5 — Guard column=0 in `handle_completions` at line 1613

```rust
let prefix_len = ((column - 1) as usize).min(text.len());
```

When `column` is 0 (i64), `column - 1` is `-1i64`, which casts to `usize::MAX`. The `.min()` clamp prevents a crash but silently produces wrong results. Add a guard before the subtraction:

```rust
let column = args.column;
if column < 1 {
    return DapResponse::error(request, "completions: column must be >= 1 (1-based)");
}
let prefix_len = ((column - 1) as usize).min(text.len());
```

#### Fix 3: M7 — Return error on malformed attach arguments at line 84

Currently:
```rust
let args: AttachRequestArguments = match request.arguments.as_ref() {
    Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
    None => AttachRequestArguments::default(),
};
```

The `None` arm is correct (attach args are optional per DAP spec). The `Some` arm should return a parse error like all other handlers:

```rust
let args: AttachRequestArguments = match request.arguments.as_ref() {
    Some(v) => match serde_json::from_value(v.clone()) {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!("Failed to parse attach arguments: {}", e);
            return DapResponse::error(request, format!("Invalid attach arguments: {e}"));
        }
    },
    None => AttachRequestArguments::default(),
};
```

### Acceptance Criteria

1. No `.expect()` in non-test code in `handlers.rs` — verify with `grep -n '\.expect(' handlers.rs | grep -v test | grep -v '// SAFETY'`
2. `handle_completions` with `column: 0` returns a DAP error response, not wrong completions
3. `handle_attach` with malformed JSON arguments returns a DAP error response, not silent defaults
4. Existing tests pass: `cargo test -p fdemon-dap`
5. `cargo clippy -p fdemon-dap` clean

### Testing

Add tests for each fix:

```rust
#[tokio::test]
async fn test_completions_column_zero_returns_error() {
    // Send completions request with column: 0
    // Assert error response
}

#[tokio::test]
async fn test_attach_malformed_arguments_returns_error() {
    // Send attach with invalid JSON arguments (e.g., wrong types)
    // Assert error response, not silent defaults
}
```

### Notes

- The `.expect()` at `stack.rs:888` reported in the review is actually in a `#[test]` function, not production code. No fix needed there.
- Scan confirmed: `handlers.rs:343` is the **only** production `.expect()` in the entire adapter directory.
