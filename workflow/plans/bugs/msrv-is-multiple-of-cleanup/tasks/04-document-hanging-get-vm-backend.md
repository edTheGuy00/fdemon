## Task: Document `HangingGetVmBackend`'s `#[allow(dead_code)]` rationale

**Objective**: Add a one-line `//` comment immediately above the `#[allow(dead_code)]` attribute on the `HangingGetVmBackend` struct, explaining why the struct is intentionally retained as test scaffolding. This makes the rationale durable to future cleanup passes that look at the attribute in isolation.

**Depends on**: None

**Estimated Time**: 0.1 hours (one comment line)

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs`: Insert a single `//` comment line between line 58 (end of existing rustdoc) and line 59 (the `#[allow(dead_code)]` attribute), or between line 59 and line 60 (the `struct HangingGetVmBackend;` declaration). The comment should briefly state that the struct is preserved as test scaffolding.

**Files Read (Dependencies):**
- None.

### Details

The current code at `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs:55-60`:

```rust
/// Backend that sleeps longer than `REQUEST_TIMEOUT` in `get_vm`.
///
/// Used to test that the timeout fires and returns an error rather than hanging.
/// The timeout is very short (1 ms) in tests because we use `tokio::time::pause`.
#[allow(dead_code)]
struct HangingGetVmBackend;
```

After the fix:

```rust
/// Backend that sleeps longer than `REQUEST_TIMEOUT` in `get_vm`.
///
/// Used to test that the timeout fires and returns an error rather than hanging.
/// The timeout is very short (1 ms) in tests because we use `tokio::time::pause`.
// Preserved as test scaffolding — see rustdoc above. Do not delete.
#[allow(dead_code)]
struct HangingGetVmBackend;
```

The wording is flexible — the goal is that a future maintainer scanning the file or running a "dead code" pass sees the `//` comment adjacent to the attribute and pauses before deleting. Acceptable variants include:
- `// Preserved as test scaffolding — see rustdoc above. Do not delete.`
- `// Test scaffolding for timeout/pause-time tests; rustdoc explains intent.`
- `// Intentional dead code: timeout-test fixture (rustdoc above).`

### Acceptance Criteria

1. `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs` contains exactly one new `//` comment line in the immediate vicinity of the `#[allow(dead_code)]` on `HangingGetVmBackend`.
2. The comment is placed between the rustdoc block (ending line 58) and the `#[allow]` attribute (line 59), OR between the attribute and the `struct` declaration. Either is acceptable; both keep the comment adjacent to the attribute.
3. The existing rustdoc block (lines 55-58) is preserved verbatim.
4. The `impl MockTestBackend for HangingGetVmBackend` block (lines 62-68) is unchanged.
5. `cargo clippy -p fdemon-dap --all-targets -- -D warnings` exits 0 (no new warnings).
6. `cargo test -p fdemon-dap` passes (no behavior change).
7. `cargo fmt --all` is clean.
8. No other lines in the file are modified.

### Testing

This is a comment-only change with no behavioral impact. Verification commands:

```bash
cargo clippy -p fdemon-dap --all-targets -- -D warnings
cargo test -p fdemon-dap
cargo fmt --all -- --check
```

All three must pass. There is no test to add — the change is purely documentary.

### Notes

- Do **not** modify the existing rustdoc. The rustdoc explains *what* the backend does; the `//` comment explains *why the attribute exists*.
- Do **not** suppress the lint at module/file scope or change the existing `#[allow(dead_code)]` to `#[allow(dead_code, ...)]`.
- This is the smallest possible change in this followup. The orchestrator should validate it cleanly without invoking the per-crate test suite for performance reasons (the change is comment-only), but the implementor still runs the full per-crate gate per the project quality bar.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a4603064c11c535cb

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs` | Added one `//` comment line between the rustdoc block and `#[allow(dead_code)]` on `HangingGetVmBackend` |

### Notable Decisions/Tradeoffs

1. **Comment placement**: Placed between the rustdoc block (line 58) and the `#[allow(dead_code)]` attribute (line 59), matching the preferred example wording from the task spec verbatim.

### Testing Performed

- `cargo clippy -p fdemon-dap --all-targets -- -D warnings` — Passed (0 warnings)
- `cargo test -p fdemon-dap` — Passed (842 unit tests, 2 doc-tests)
- `cargo fmt --all -- --check` — Passed (no formatting changes needed)

### Risks/Limitations

1. **None**: This is a comment-only change with no behavioral impact.
