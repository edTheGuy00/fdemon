## Task: Clean up clippy warnings in `fdemon-app`

**Objective**: Resolve all clippy warnings in `fdemon-app` so `cargo clippy -p fdemon-app --all-targets -- -D warnings` exits 0.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):** (under `crates/fdemon-app/src/`)
- `actions/native_logs.rs` — 1 warning.
- `actions/network.rs` — 1 warning.
- `actions/performance.rs` — 3 warnings.
- `actions/vm_service.rs` — 1 warning.
- `config/settings.rs` — ~16 warnings (likely `field_reassign_with_default` cluster).
- `handler/devtools/debug.rs` — 1 warning.
- `handler/helpers.rs` — 2 warnings (likely `clone_on_copy` for `DartDefinesPane`).
- `handler/new_session/launch_context.rs` — ~44 warnings (heaviest single file in this crate; mostly `field_reassign_with_default`).
- `handler/settings_dart_defines.rs` — 3 warnings.
- `handler/tests.rs` — ~10 warnings (likely `bool_assert_comparison`, `assertions_on_constants`).
- `new_session_dialog/target_selector_state.rs` — 7 warnings.
- `session/debug_state.rs` — 4 warnings.
- `session/network.rs` — ~14 warnings.
- `session/performance.rs` — 2 warnings.
- `session/tests.rs` — 1 warning (`clippy::module_inception` — `mod tests` inside `tests.rs`).
- `settings_items.rs` — 2 warnings (`field_reassign_with_default` cluster).
- `spawn.rs` — 1 warning.
- `state.rs` — ~12 warnings.
- `watcher/mod.rs` — 1 warning.

**Files Read (Dependencies):**
- None.

### Warning Inventory (79 total)

| Lint | Count | Notes |
|------|-------|-------|
| `clippy::field_reassign_with_default` | 47 | Bulk of the work; concentrated in `handler/new_session/launch_context.rs`, `config/settings.rs`, `state.rs`, `session/network.rs`. |
| `clippy::bool_assert_comparison` | 13 | `assert_eq!(x, true/false)` → `assert!(x)` / `assert!(!x)` (mostly `handler/tests.rs`). |
| `clippy::assertions_on_constants` | 5 | `assert!(true)` — remove or replace with the actual assertion the test was meant to make (re-read context!). |
| `clippy::type_complexity` | 2 | Extract `Arc<Mutex<…>>` chains into `type` aliases. |
| `clippy::clone_on_copy` | 2 | `.clone()` on `DartDefinesPane` (which is `Copy`); remove the `.clone()`. |
| `clippy::useless_vec` | 2 | `vec![…]` → `[…]` array literal. |
| `clippy::items_after_test_module` | 2 | Move non-test items above the `#[cfg(test)] mod tests { … }` block. |
| `clippy::while_let_loop` | 1 | `loop { match … }` → `while let`. |
| `clippy::cloned_ref_to_slice_refs` | 1 | `.cloned()` on a slice ref → `slice::from_ref(…)`. |
| `clippy::module_inception` | 1 | `mod tests` inside `session/tests.rs` — annotate with `#[allow(clippy::module_inception)]`. |
| `clippy::single_element_loop` | 1 | `for x in [val]` → direct use of `val`. |
| `clippy::map_flatten` | 1 | `.map(…).flatten()` on `Option` → `.and_then(…)`. |

### Procedure

1. From the repo root, apply mechanical fixes:
   ```bash
   cargo clippy --fix -p fdemon-app --all-targets --allow-dirty
   ```
   Clippy reports 21 auto-fix suggestions for this crate; expect this pass to clear roughly a quarter of the warnings.
2. Hand-fix what remains. Work file-by-file in this order (largest first to fail fast on tricky cases):
   - **`handler/new_session/launch_context.rs`** (~44) — predominantly `field_reassign_with_default`. Convert sequential `Default::default()` + assignment blocks to struct literals (see task 04 for an example). Conditional/interleaved assignments must stay as-is.
   - **`config/settings.rs`** (~16) — same pattern, fewer sites.
   - **`session/network.rs`** (~14) — same pattern.
   - **`state.rs`** (~12) — same pattern.
   - **`handler/tests.rs`** (~10) — mix of `bool_assert_comparison` and `assertions_on_constants`. For each `assert!(true)`:
     - **Re-read the surrounding test context.** A bare `assert!(true)` is often a placeholder where the original assertion was deleted or commented out. Restore the intended assertion if possible (e.g., the test name hints at what to check); only remove the line if it is genuinely a no-op.
     - If the intent is unclear, leave the line and add `#[allow(clippy::assertions_on_constants)]` plus a `// FIXME: placeholder; see clippy-rust-191-cleanup` comment so the assertion is flagged for follow-up.
   - **`session/tests.rs`** — add `#[allow(clippy::module_inception)]` directly above the `mod tests {` line (line ~4). Do not rename the inner module; the convention `tests.rs` ➜ `mod tests` is used elsewhere in the crate.
   - **`type_complexity` (2 sites)** — extract private `type` aliases at the top of the affected modules. See task 03 for guidance.
   - **`items_after_test_module` (2)** — move the offending non-test items (consts/fns/types after `#[cfg(test)]`) to before the `mod tests` block. This keeps the test module strictly at the bottom, which is the pre-existing convention.
   - **`clone_on_copy` (2)** — remove the `.clone()` calls on `DartDefinesPane` instances in `handler/helpers.rs`.
   - **`map_flatten`** on `Option` — rewrite `.map(f).flatten()` as `.and_then(f)` (clippy's hint for `Option` is `and_then`, not `flat_map`).
   - **`cloned_ref_to_slice_refs`** — replace the `.cloned()` call with `std::slice::from_ref(&value)` per clippy's suggestion.
   - **`single_element_loop`** — unroll the trivial `for x in [val] { … }` into the single-iteration form.
3. Run the per-crate gate:
   ```bash
   cargo clippy -p fdemon-app --all-targets -- -D warnings
   ```
4. Run unit tests:
   ```bash
   cargo test -p fdemon-app
   ```
   Existing 1,511 unit tests must continue to pass.
5. Format:
   ```bash
   cargo fmt --all
   ```

### Acceptance Criteria

1. `cargo clippy -p fdemon-app --all-targets -- -D warnings` exits 0.
2. `cargo test -p fdemon-app` passes (no regressions; 1,511 unit tests).
3. Diff is limited to files under `crates/fdemon-app/src/`.
4. No `assert!(true)` lines remain unless explicitly annotated with `#[allow(clippy::assertions_on_constants)]` and a `FIXME` comment.
5. The `mod tests` block in `session/tests.rs` is preserved (only annotated with `#[allow(clippy::module_inception)]`).
6. `clippy::type_complexity` aliases are private (file-local `type`).
7. No public API changes — all type aliases, `#[allow]` annotations, and struct-literal conversions stay internal.

### Notes

- This is the largest single task. Consider committing per-file as you go to keep the diff reviewable.
- `assertions_on_constants` deserves extra care — `assert!(true)` lines are often dead test scaffolding. Treat each as a small investigation: check `git blame` to understand the original intent, restore the real assertion if possible, otherwise annotate-and-defer rather than silently delete.
- For `field_reassign_with_default`, the lint only fires when the post-`default()` assignments are unconditional and consecutive. Conditional or interleaved assignments must remain in their existing form — do not force them into a struct literal.

---

## Completion Summary

**Status:** Not Started
**Branch:** _to be filled by implementor_

### Files Modified

| File | Changes |
|------|---------|
| _tbd_ | _tbd_ |

### Notable Decisions/Tradeoffs

_tbd_

### Testing Performed

- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — _tbd_
- `cargo test -p fdemon-app` — _tbd_
- `cargo fmt --all -- --check` — _tbd_

### Risks/Limitations

_tbd_
