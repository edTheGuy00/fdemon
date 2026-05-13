## Task: Code Style Sweep (Phase 3, Bundle)

**Objective**: Address four style/standards issues in the inspector module: extract magic strings as named constants, convert mixed format-string `tracing!` calls to structured fields, rename 5 non-conforming test names, and correct one misleading docstring.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/inspector/mod.rs` — extract VM object group constants
- `crates/fdemon-app/src/actions/inspector/widget_tree.rs` — convert format-string `tracing!` calls to structured fields; rename non-conforming tests; correct `try_fetch_widget_tree` docstring

**Files Read (Dependencies):**
- None

### Details

**11a. Extract magic strings as constants:**

`actions/inspector/mod.rs` has at least four occurrences of `"fdemon-inspector-1"` and `"devtools-layout"` (lines ~114, 350, 499). Extract:

```rust
/// VM object group for the widget inspector. Scopes `valueId` references
/// returned by `getRootWidgetTree`.
const INSPECTOR_OBJECT_GROUP: &str = "fdemon-inspector-1";

/// VM object group for the layout explorer. Scopes `valueId` references
/// returned by `getLayoutExplorerNode`.
const LAYOUT_OBJECT_GROUP: &str = "devtools-layout";
```

Replace all string-literal usages with the constants.

**11b. Convert mixed-style tracing calls in `widget_tree.rs`:**

Lines 96-101, 118-123, 143-149 currently use positional format strings. Convert to structured fields matching the project standard:

```rust
// Before:
tracing::debug!(
    "isWidgetTreeReady timed out for session {} (poll {}/{}), treating as not ready",
    session_id, attempt, config.attempts,
);

// After:
tracing::debug!(
    session_id = %session_id,
    attempt = attempt,
    max_polls = config.attempts,
    "isWidgetTreeReady timed out; treating as not ready"
);
```

Apply to all three sites. Match the structured style already used elsewhere in the same function (e.g., line 80-85).

**11c. Rename non-conforming test names:**

Per `docs/CODE_STANDARDS.md` and `docs/REVIEW_FOCUS.md`, test names follow `test_<function>_<scenario>_<expected_result>`. Five tests in `widget_tree.rs` violate this:

| Current | Renamed |
|---------|---------|
| `readiness_poll_config_defaults_match_spec` | `test_readiness_poll_config_default_matches_spec` |
| `readiness_poll_config_custom_values` | `test_readiness_poll_config_custom_values_are_stored` |
| `poll_exhaustion_returns_ok_not_error` | `test_poll_widget_tree_ready_exhausted_returns_unit` |
| `poll_with_zero_attempts_returns_immediately` | `test_poll_widget_tree_ready_zero_attempts_returns_immediately` |
| `poll_respects_custom_attempts_and_interval` | `test_poll_widget_tree_ready_custom_attempts_bound_loop` |

**11d. Correct `try_fetch_widget_tree` docstring:**

The docstring lists "method not found" and "transient error" as distinct cases 2 and 3, but the code merges them via `is_transient_error` (lines 195-208). Update the docstring to reflect actual behavior — both transient errors and method-not-found currently take the same fallback to `getRootWidgetSummaryTree`. Either:
- Remove the artificial distinction in the doc, OR
- Keep the distinction in the doc but split the code path (likely YAGNI for now).

Recommended: remove the distinction. Update the doc to describe the actual unified fallback path.

### Acceptance Criteria

1. `git grep '"fdemon-inspector-1"\|"devtools-layout"' crates/fdemon-app/src/actions/inspector/` returns matches *only* inside the two `const` declarations.
2. All `tracing::debug!`/`info!`/`warn!` calls in `widget_tree.rs` use structured fields (no positional format strings for the new code paths from this fix).
3. All 5 non-conforming test names are renamed.
4. `try_fetch_widget_tree` docstring accurately describes the unified transient/method-not-found fallback.
5. All CI quality gates pass.

### Testing

No new tests. Verify that:
- Renamed tests still run and pass (`cargo test test_readiness_poll_config_default_matches_spec` etc.)
- `cargo clippy --workspace --all-targets -- -D warnings` is green
- No compilation errors from missed call-site updates after const extraction

### Notes

- Be careful with structured-field conversion — `%session_id` is Display formatting, `?session_id` is Debug. Match the existing style in `widget_tree.rs` (mostly `%` for IDs, `?` for complex types).
- The const extraction may surface other call sites in test code that pass string literals. Update them too if their meaning is the same group name.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/inspector/mod.rs` | Added `INSPECTOR_OBJECT_GROUP` and `LAYOUT_OBJECT_GROUP` constants; replaced all 3 code-site string literals with those constants |
| `crates/fdemon-app/src/actions/inspector/widget_tree.rs` | Converted 5 format-string tracing calls to structured fields; renamed 5 tests; updated `try_fetch_widget_tree` docstring; replaced test literal `"fdemon-inspector-1"` with `super::super::INSPECTOR_OBJECT_GROUP` |

### Notable Decisions/Tradeoffs

1. **Docstring literals in mod.rs preserved**: The two docstring mentions of `"fdemon-inspector-1"` and `"devtools-layout"` (in `///` comments) were left as string literals — they document the actual string value of the constants, which is the right content for readers. Only code-site usages were replaced with the constants.
2. **Two extra tracing calls converted**: Lines ~203 and ~226 in `try_fetch_widget_tree` also used positional format strings; these were converted to structured fields to satisfy the "all calls" wording of acceptance criterion 2 and maintain consistency.
3. **Test literal replaced with `super::super::` path**: The test in `widget_tree.rs` that passed `"fdemon-inspector-1"` to `try_fetch_widget_tree` was updated to use `super::super::INSPECTOR_OBJECT_GROUP` — `super` = `widget_tree` module, `super::super` = `inspector` module where the constant lives. This compiles cleanly since child modules can access parent private items in Rust.

### Testing Performed

- `cargo fmt --all -- --check` - Passed (no output = clean)
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app --lib` - Passed (2208 tests)
- `cargo test -p fdemon-app "test_readiness_poll_config"` - Passed (2 renamed tests)
- `cargo test -p fdemon-app "test_poll_widget_tree_ready"` - Passed (3 renamed tests)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **None significant**: All changes are pure renaming, constant extraction, and style normalization — no behavioral changes.
