# Task 09: Render Tests Stale-Comment Cleanup

## Goal

Update the stale comment in `crates/fdemon-tui/src/render/tests.rs:87-92` that predicted Phase-5 changes (now landed) and replace with a precise note about the current 120×24 Normal-mode shortcut-region count.

## Background

`crates/fdemon-tui/src/render/tests.rs:87-92`:
```rust
// Phase 5: modal overlay regions (tag-filter, Settings panel internals) may push
// additional entries into the registry. Update this exact-count assertion to
// `>= 6` (or split into per-source counts) when those regions land.
assert_eq!(shortcut_msgs.len(), 6, "exactly six shortcut regions");
```

Phase 5 has landed. The 120×24 Normal-mode render path is unaffected by Phase 5 modal regions (no modal is open in this test scenario — `state.ui_mode == UiMode::Normal`, no `tag_filter_visible`, etc.). The stale comment misleads future maintainers into thinking the assertion needs updating when modals come online; in fact, it doesn't (modal regions are conditional on `ui_mode`).

## Files

**Modify:**
- `crates/fdemon-tui/src/render/tests.rs` — comment-only edit at lines 87-92

## Plan

1. **Replace the stale comment** with a clear post-Phase-5 note:
   ```rust
   // 120×24 Normal mode renders: header brackets `[r] [d] [D] [s] [c] [q]`
   // (six z=0 regions) + log-row regions if any logs exist. Modal regions
   // (NewSessionDialog z=1, ConfirmDialog z=1, TagFilter overlay z=1, Settings
   // z=1) are NOT in this registry — they are only registered when the
   // corresponding `UiMode` is active. Phase 5/5.5 do not change this baseline.
   assert_eq!(shortcut_msgs.len(), 6, "exactly six shortcut regions in 120×24 Normal mode");
   ```

2. **No other changes** — the assertion itself is correct and remains.

3. **Quality gates** (the test file was modified, so confirm it still passes):
   ```bash
   cargo test -p fdemon-tui render::tests
   cargo fmt --all -- --check
   ```

## Acceptance Criteria

- [ ] Comment at `crates/fdemon-tui/src/render/tests.rs:87-92` no longer claims Phase 5 work is pending.
- [ ] Comment accurately describes the 120×24 Normal-mode shortcut region count and why modals are not present.
- [ ] Test still passes.
- [ ] `cargo fmt --all -- --check` passes.

## Notes

- This is the smallest task in 5.5 (~15min) and exists primarily to keep the test file's documentation accurate. Skipping it leaves a breadcrumb that misleads future readers.
- T01 may have added new tests to this file for modal-precedence coverage. T01's writes go to `handler/tests.rs`, not `render/tests.rs` — verify.
- T01 ↔ T09: no overlap. T01 writes `handler/tests.rs`; T09 writes `render/tests.rs`.
- T05 may also touch `widgets/settings_panel/tests.rs` — also no overlap with T09.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/render/tests.rs` | Replaced stale Phase-5-pending comment with accurate post-Phase-5 note; reformatted `assert_eq!` to multi-line form for rustfmt compliance |

### Notable Decisions/Tradeoffs

1. **Multi-line assert_eq!**: The updated assertion message string caused the single-line `assert_eq!` to exceed rustfmt's default line width (100 chars). Reformatted to the three-argument multi-line form to satisfy `cargo fmt --all -- --check`.

### Testing Performed

- `cargo test -p fdemon-tui render::tests` - Passed (26 tests)
- `cargo fmt --all -- --check` - Passed

### Risks/Limitations

None — comment-only change with formatting fix.
