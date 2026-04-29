## Task: Restore MSRV compliance in `fdemon-dap` (`breakpoints.rs`)

**Objective**: Replace the single `is_multiple_of` call site in `fdemon-dap::adapter::breakpoints::evaluate_hit_condition()` with `% N == 0` and suppress the resulting clippy lint at function scope, restoring compatibility with the workspace's declared MSRV (`1.77.2`).

**Depends on**: None

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/breakpoints.rs`: Edit the `evaluate_hit_condition()` function around line 692. Replace `hit_count.is_multiple_of(n)` with `hit_count % n == 0`. Add `#[allow(clippy::manual_is_multiple_of)]` plus the MSRV justification comment on the `evaluate_hit_condition` function.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/devtools/network/tests.rs:11-12`: Use as the reference for the exact comment wording and attribute placement.

### Details

The current code at `crates/fdemon-dap/src/adapter/breakpoints.rs:682-696` (relevant excerpt):

```rust
pub fn evaluate_hit_condition(hit_count: u64, condition: &str) -> bool {
    let condition = condition.trim();

    // Handle modulo operator: "% N" or "% N == 0"
    if let Some(rest) = condition.strip_prefix('%') {
        // Trim the rest and extract the modulus value (ignore trailing "== 0").
        let rest = rest.trim();
        // Accept "% N" or "% N == 0"
        let modulus_str = rest.split_whitespace().next().unwrap_or("");
        return match modulus_str.parse::<u64>() {
            Ok(n) if n > 0 => hit_count.is_multiple_of(n),
            // Parse failure or zero divisor → safe default: stop
            _ => true,
        };
    }
    // ...
}
```

After the fix:

```rust
// MSRV guard: `is_multiple_of` requires Rust 1.87; MSRV is 1.77.2 — suppress the lint.
#[allow(clippy::manual_is_multiple_of)]
pub fn evaluate_hit_condition(hit_count: u64, condition: &str) -> bool {
    let condition = condition.trim();

    // Handle modulo operator: "% N" or "% N == 0"
    if let Some(rest) = condition.strip_prefix('%') {
        // Trim the rest and extract the modulus value (ignore trailing "== 0").
        let rest = rest.trim();
        // Accept "% N" or "% N == 0"
        let modulus_str = rest.split_whitespace().next().unwrap_or("");
        return match modulus_str.parse::<u64>() {
            Ok(n) if n > 0 => hit_count % n == 0,
            // Parse failure or zero divisor → safe default: stop
            _ => true,
        };
    }
    // ...
}
```

Note: Place the MSRV-guard comment and `#[allow]` attribute immediately before `pub fn evaluate_hit_condition`. If the function has an existing rustdoc comment block above it, the MSRV-guard `//` comment goes between the rustdoc and the `#[allow]` attribute. Do not modify the existing rustdoc, the `// Handle modulo operator: …` comment, or any other surrounding logic.

### Acceptance Criteria

1. `crates/fdemon-dap/src/adapter/breakpoints.rs` no longer contains `is_multiple_of` (verify with `grep -n 'is_multiple_of' crates/fdemon-dap/src/adapter/breakpoints.rs` → no matches).
2. `evaluate_hit_condition` carries `#[allow(clippy::manual_is_multiple_of)]` preceded by the MSRV justification comment.
3. `cargo clippy -p fdemon-dap --all-targets -- -D warnings` exits 0.
4. `cargo test -p fdemon-dap` passes — breakpoint hit-condition tests (modulo evaluation) continue to behave identically.
5. `cargo fmt --all` is clean.
6. No other lines in `breakpoints.rs` are modified.
7. The `n > 0` guard in `Ok(n) if n > 0 =>` is preserved (this is what makes `% n` panic-safe).

### Testing

Existing breakpoint tests already exercise `evaluate_hit_condition` with modulo conditions. Search for them:

```bash
grep -rn "evaluate_hit_condition" crates/fdemon-dap/src/
grep -rn "% 2\|% 3\|% 5\|% 10" crates/fdemon-dap/src/adapter/
```

Verify any test that asserts modulo-based hit conditions still passes. The rewrite is observably identical for all `n > 0`.

### Notes

- The `n > 0` guard prevents division-by-zero panic — the rewrite is panic-safe because of the surrounding code, not because of the rewrite itself.
- This task does **not** address the `HangingGetVmBackend` `#[allow(dead_code)]` rationale comment — that's task 04 (separate file: `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs`).
- Do **not** suppress the lint at module/file scope.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/breakpoints.rs` | Replaced `hit_count.is_multiple_of(n)` with `hit_count % n == 0`; added MSRV guard comment and `#[allow(clippy::manual_is_multiple_of)]` before `evaluate_hit_condition` |

### Notable Decisions/Tradeoffs

1. **Attribute placement**: The MSRV guard comment and `#[allow]` attribute were placed between the existing rustdoc block and the `pub fn` declaration, exactly as specified in the task and matching the reference pattern in `fdemon-tui/src/widgets/devtools/network/tests.rs:11-12`.
2. **No other changes**: Only the two specified modifications were made — the `is_multiple_of` call and the attribute/comment addition. All other logic, comments, and the `n > 0` guard were preserved unchanged.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo clippy -p fdemon-dap --all-targets -- -D warnings` - Passed (0 warnings)
- `cargo test -p fdemon-dap` - Passed (842 unit tests + 2 doc-tests, 0 failed)

### Risks/Limitations

1. **None**: The rewrite from `is_multiple_of(n)` to `% n == 0` is semantically identical for all `n > 0`, and the `n > 0` guard prevents division-by-zero. No behavioral change.
