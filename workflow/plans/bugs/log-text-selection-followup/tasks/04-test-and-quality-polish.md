## Task: Test + quality polish (vacuous assertion, EXCEPTION annotation, magic number, focused test, Unicode contract)

**Objective:** Bundle five small code-quality fixes that share no file overlap with each other but do collectively improve test signal and coding-standard conformance:

1. Rewrite the vacuous second assertion in `test_status_info_renders_mouse_off_badge`.
2. Conform the EXCEPTION annotation in `handler/mouse/mod.rs:130` to the verbatim style required by `CODE_STANDARDS.md`.
3. Extract `60` magic number into a named `COPY_TOAST_PREVIEW_CHARS` constant in `handler/update.rs`.
4. Add a focused unit test for `resolve_entry_text` in `handler/tests.rs`.
5. Document the Unicode-scalar contract on `truncate_with_ellipsis`.

**Depends on:** None

**Agent:** implementor

**Estimated time:** 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`: rewrite vacuous assertion in `test_status_info_renders_mouse_off_badge` (around line 2523-2526).
- `crates/fdemon-app/src/handler/mouse/mod.rs`: update EXCEPTION annotation comment around line 130 to match required style.
- `crates/fdemon-app/src/handler/update.rs`: add `const COPY_TOAST_PREVIEW_CHARS: usize = 60;` and reference it; add doc-comment on `truncate_with_ellipsis` describing the Unicode-scalar contract.
- `crates/fdemon-app/src/handler/tests.rs`: add `test_resolve_entry_text_*` focused unit tests near the existing copy-message tests (around line 12792-12888 area).

**Files Read (Dependencies):**
- `docs/CODE_STANDARDS.md`: EXCEPTION annotation style (Region Registry Pattern → Annotation requirement).
- `docs/REVIEW_FOCUS.md`: cross-referenced from the EXCEPTION annotation.
- `crates/fdemon-app/src/handler/update.rs`: source of `resolve_entry_text` and `truncate_with_ellipsis` (read to understand signatures for the new test).

### Details

#### 1. Vacuous assertion rewrite

Current code at `crates/fdemon-tui/src/widgets/log_view/tests.rs:2523-2526`:

```rust
assert!(
    !term.buffer_contains("[mouse]") || term.buffer_contains("[mouse-off]"),
    "Status bar must not show plain '[mouse]' without '-off' when inactive"
);
```

Once the first assertion has confirmed `[mouse-off]` is present (lines 2519-2522), the second assertion is `false || true = true` always, so it cannot detect a regression where `[mouse]` (without `-off`) appears. Rewrite to actually check that the substring `[mouse]` does NOT appear adjacent to a `]` (vs `-`):

```rust
// `[mouse-off]` contains the substring `[mouse`, so a naive substring search is
// always true. Check that the string `[mouse]` (closing bracket, no `-off`)
// is absent from the rendered buffer. We scan for the literal "[mouse]" 7-char
// sequence; the off-state badge `[mouse-off]` would NOT match this.
assert!(
    !term.buffer_contains("[mouse]"),
    "Status bar must not show plain '[mouse]' (the on-state badge) when capture is off; \
     full buffer:\n{}",
    term.buffer_dump()  // or whatever helper renders the full buffer
);
```

If `TestTerminal` does not have `buffer_dump()`, just include the regular failure message — the goal is to make the assertion non-vacuous. If even substring matching is broken (because `[mouse-off]` *does* contain `[mouse` as a 6-char substring, just not the 7-char `[mouse]`), confirm by reading the `buffer_contains` implementation in the test infrastructure. The 7-char `[mouse]` is only present in the on-state, so the assertion is correct.

#### 2. EXCEPTION annotation conformance

Current code at `crates/fdemon-app/src/handler/mouse/mod.rs:130`:

```rust
// EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3
```

Replace with the verbatim style required by `CODE_STANDARDS.md` "Region Registry Pattern → Annotation requirement":

```rust
// EXCEPTION (TEA): mouse_regions is a render-hint cell. See docs/CODE_STANDARDS.md
// "Region Registry Pattern" and docs/REVIEW_FOCUS.md approved-exceptions list.
```

The required style includes the `(TEA)` qualifier and cross-references to BOTH `CODE_STANDARDS.md` (named section, not "Principle 3") and `REVIEW_FOCUS.md`.

#### 3. Magic-number extraction

Current code at `crates/fdemon-app/src/handler/update.rs:2887` (approximately — search for `truncate_with_ellipsis(&entry_text, 60)`):

```rust
let preview = truncate_with_ellipsis(&entry_text, 60);
```

Add a named constant adjacent to the handler arm (or near the top of the file, alongside other module-level constants):

```rust
/// Maximum Unicode scalar values shown in the "Copied: …" toast preview.
/// Documented user-facing in docs/MOUSE.md; keep in sync if changed.
const COPY_TOAST_PREVIEW_CHARS: usize = 60;
```

Reference from both the call site and the test bound:

```rust
let preview = truncate_with_ellipsis(&entry_text, COPY_TOAST_PREVIEW_CHARS);
```

In the test `test_copy_message_truncates_preview_to_60_chars` (in `handler/tests.rs`), update the assertion bound from `<= 61` to `<= COPY_TOAST_PREVIEW_CHARS + 1`. Import the constant if it's `pub(crate)` or restate the value in a comment if it isn't (prefer the former).

#### 4. `resolve_entry_text` focused unit test

Add a dedicated test for `resolve_entry_text` near the existing copy-message tests in `crates/fdemon-app/src/handler/tests.rs`. The function must be exposed at least at `pub(crate)` level for direct testing (if it's currently `fn`, change to `pub(crate) fn`).

Three test cases:

```rust
#[test]
fn test_resolve_entry_text_no_active_session() {
    let state = AppState::new();
    // No session created.
    let result = resolve_entry_text(&state, 42);
    assert!(result.is_empty(), "Expected empty string for no active session, got: {result:?}");
}

#[test]
fn test_resolve_entry_text_missing_entry_id() {
    let (state, _session_id, _entry_id) = make_state_with_log_entry("real entry");
    let missing_id: u64 = 999_999;
    let result = resolve_entry_text(&state, missing_id);
    assert!(result.is_empty(), "Expected empty string for missing entry, got: {result:?}");
}

#[test]
fn test_resolve_entry_text_matching_entry() {
    let (state, _session_id, entry_id) = make_state_with_log_entry("Hello from Flutter");
    let result = resolve_entry_text(&state, entry_id);
    assert!(
        result.contains("Hello from Flutter"),
        "Expected resolved text to contain log message, got: {result:?}"
    );
}
```

Reuse the `make_state_with_log_entry` helper that already exists in `tests.rs` (added by the parent plan's task 06).

#### 5. `truncate_with_ellipsis` Unicode contract

Add a doc-comment to `truncate_with_ellipsis`:

```rust
/// Truncate `s` to at most `max_chars` Unicode scalar values, appending `…` if
/// truncated. Operates on scalar values, NOT grapheme clusters — a flag emoji
/// or family-zwj sequence at exactly the boundary may be split mid-cluster.
/// Acceptable here because the function is used only for status-toast previews
/// where occasional mid-cluster truncation is cosmetic. If used in user-visible
/// output where grapheme integrity matters, swap to `unicode-segmentation`.
```

### Acceptance Criteria

1. `test_status_info_renders_mouse_off_badge` second assertion is no longer vacuous — it can fail if `[mouse]` (the on-state badge) appears in the off-state buffer.
2. EXCEPTION annotation in `handler/mouse/mod.rs` matches the verbatim style required by `CODE_STANDARDS.md` (`(TEA)` qualifier, both cross-references).
3. `grep -n '\b60\b' crates/fdemon-app/src/handler/update.rs` returns no matches in the toast-preview context — the literal is replaced by `COPY_TOAST_PREVIEW_CHARS`.
4. Three new `test_resolve_entry_text_*` tests exist and pass.
5. `truncate_with_ellipsis` has a doc-comment that mentions the Unicode-scalar (vs grapheme) contract.
6. `cargo fmt --all -- --check` passes.
7. `cargo clippy --workspace --all-targets -- -D warnings` passes.
8. `cargo test --workspace` passes — all existing tests + the new ones.

### Testing

Run `cargo test --workspace`. The new `test_resolve_entry_text_*` tests should appear in the fdemon-app output. The rewritten `test_status_info_renders_mouse_off_badge` should still pass (the on-state badge is genuinely absent in the off-state).

### Notes

- This task touches `handler/tests.rs`. **Task 05 also touches `handler/tests.rs`.** The orchestrator will run these two tasks sequentially on the current branch, not in parallel worktrees.
- The vacuous-assertion fix may require checking what `term.buffer_contains` actually does. If it's an exact-substring search, `[mouse]` (with closing bracket) will not match `[mouse-off]` (with `-`), so the rewritten assertion is correct. Verify by reading the `TestTerminal` implementation before submitting.
- Do NOT change `truncate_with_ellipsis`'s implementation — only its doc-comment.
