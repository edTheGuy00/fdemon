## Task: Clean up clippy warnings in `fdemon-dap`

**Objective**: Resolve all clippy warnings in `fdemon-dap` so `cargo clippy -p fdemon-dap --all-targets -- -D warnings` exits 0.

**Depends on**: None

**Estimated Time**: 1–1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/threads.rs` — replace 5 manual range checks (`x >= 1000 && x < 2000`) with `(1000..2000).contains(&x)` and similar around lines 777, 797, 801, 983, 984.
- `crates/fdemon-dap/src/adapter/tests/call_service.rs` — fix one `unused variable: rx` and one `variable does not need to be mutable` (lines 101, 108).
- `crates/fdemon-dap/src/adapter/tests/restart_frame.rs` — same pattern at lines 35, 39 (one `rx` unused, one unneeded `mut`).
- `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs` — add `#[allow(dead_code)]` to `struct HangingGetVmBackend` (line 59); fix any `rx` unused / `mut` warnings the file emits.
- `crates/fdemon-dap/src/adapter/tests/stack_scopes_variables.rs` — bulk-fix 24 occurrences of paired `unused variable: rx` + `variable does not need to be mutable` (clippy `--fix` should auto-resolve almost all; verify around lines 1762, 1789, 1829, 1870, 1936, 1958, 2005, etc.).
- `crates/fdemon-dap/src/adapter/tests/update_debug_options.rs` — fix one `unused variable: rx` at line 52.
- Plus any `clippy::type_complexity` extractions in `threads.rs` or test files (5 occurrences total): introduce private `type` aliases at the top of the affected module.

**Files Read (Dependencies):**
- None.

### Warning Inventory (35 total)

| Lint | Count | Notes |
|------|-------|-------|
| `unused_variable: rx` | 12 | Test channel patterns where only the sender is used; prefix `_rx` (or `_`). |
| `variable does not need to be mutable` | 12 | Pairs with the `rx` warnings; `let mut (tx, rx)` → `let (tx, rx)` (or `_rx`). |
| `clippy::type_complexity` | 5 | Extract `Arc<Mutex<…>>` chains into module-local `type` aliases. |
| `clippy::manual_range_contains` | 5 | `x >= 1000 && x < 2000` → `(1000..2000).contains(&x)` (in `threads.rs`). |
| `dead_code: HangingGetVmBackend` | 1 | Test struct never constructed; `#[allow(dead_code)]` on the struct. |

### Procedure

1. From the repo root, apply mechanical fixes:
   ```bash
   cargo clippy --fix -p fdemon-dap --all-targets --allow-dirty
   ```
   This handles `manual_range_contains`, `unused_variable`, and the paired `mut` warnings (clippy reports 17 auto-fixable suggestions for this crate).
2. Hand-fix what remains:
   - **`type_complexity` (5)**: locate each `^^^…` highlight from clippy output and define a `type` alias near the top of the file (or just above the function). Aliases stay private. Example:
     ```rust
     type SharedRequests = std::sync::Arc<std::sync::Mutex<Vec<(u32, String)>>>;
     ```
   - **`dead_code` on `HangingGetVmBackend`**: add `#[allow(dead_code)]` immediately above the struct declaration in `request_timeouts_events.rs:59`. Do not delete — the type is intentional test scaffolding (referenced by name only in some compile-checks).
3. Run the per-crate gate:
   ```bash
   cargo clippy -p fdemon-dap --all-targets -- -D warnings
   ```
4. Run unit tests:
   ```bash
   cargo test -p fdemon-dap
   ```
5. Format:
   ```bash
   cargo fmt --all
   ```

### Acceptance Criteria

1. `cargo clippy -p fdemon-dap --all-targets -- -D warnings` exits 0.
2. `cargo test -p fdemon-dap` passes (no regressions).
3. Diff is limited to files under `crates/fdemon-dap/src/adapter/`.
4. `HangingGetVmBackend` is preserved (only annotated with `#[allow(dead_code)]`, not deleted).
5. Each `type_complexity` extraction is private (no `pub type …`).

### Notes

- Run `cargo clippy --fix` once and inspect the diff — it usually handles the bulk of `rx`/`mut` pairs in one pass. Manually re-run for any sites it skips.
- For `unused_variable: rx`, prefer `_rx` over `_` so the original binding name remains as documentation.
- The `type_complexity` aliases should be named after their semantic role (e.g., `SharedRequests`, `EventListeners`), not their structural shape.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/threads.rs` | Auto-fixed 5 `manual_range_contains` warnings: `x >= 1000 && x < 2000` → `(1000..2000).contains(&x)` at lines 777, 797, 801, 983, 984 |
| `crates/fdemon-dap/src/adapter/tests/stack_scopes_variables.rs` | Renamed 12 occurrences of `rx` → `_rx` (all in `let (mut adapter, rx, thread_id)` patterns where receiver channel was unused); auto-fix had already removed `mut` from these |
| `crates/fdemon-dap/src/adapter/tests/call_service.rs` | Added `type SharedCallLog = Arc<Mutex<Vec<(String, Option<serde_json::Value>)>>>` alias; updated `RecordingCallServiceBackend` struct and `new()` return type to use it |
| `crates/fdemon-dap/src/adapter/tests/restart_frame.rs` | Added `type SharedResumeLog = Arc<Mutex<Vec<(String, Option<StepMode>, Option<i32>)>>>` alias; updated `RecordingBackend` struct and `new()` return type to use it |
| `crates/fdemon-dap/src/adapter/tests/update_debug_options.rs` | Added `type SharedDebuggabilityLog = Arc<Mutex<Vec<(String, String, bool)>>>` alias; updated `LibraryDebuggableMock` struct and `new()` return type to use it |
| `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs` | Added `#[allow(dead_code)]` to `HangingGetVmBackend` struct |

### Notable Decisions/Tradeoffs

1. **`cargo clippy --fix` first**: Applied the automated fix pass first to handle `manual_range_contains` and the `unused_mut` pairs in test files other than `stack_scopes_variables.rs`. The auto-fixer removed `mut` from `rx` bindings but did not prefix them with `_`, so remaining `unused_variable: rx` errors in `stack_scopes_variables.rs` required a manual `replace_all` edit.
2. **Type alias naming follows semantic role**: `SharedCallLog`, `SharedResumeLog`, and `SharedDebuggabilityLog` are named after what they record, not their structural shape — consistent with the task's guidance.
3. **`HangingGetVmBackend` preserved**: Added `#[allow(dead_code)]` only; the struct and its `MockTestBackend` impl are intact as intentional scaffolding.

### Testing Performed

- `cargo clippy -p fdemon-dap --all-targets -- -D warnings` — Passed (exit 0, no warnings)
- `cargo test -p fdemon-dap` — Passed (842 unit tests + 2 doc tests)
- `cargo fmt --all -- --check` — Passed (exit 0)
