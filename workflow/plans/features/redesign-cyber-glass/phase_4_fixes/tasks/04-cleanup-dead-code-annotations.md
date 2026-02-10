## Task: Clean Up Dead Code Annotations

**Objective**: Remove `#[allow(dead_code)]` from 11 style functions that are actively used, and remove 4 genuinely dead items.

**Depends on**: None

**Severity**: Major (code hygiene — suppresses future dead code warnings)

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/styles.rs`: Remove annotations, delete dead code
- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`: Remove `#[allow(dead_code)]` from `settings` field if it's now used (for IconMode)

### Details

#### Remove `#[allow(dead_code)]` from LIVE functions (11 items)

These functions are actively called from `mod.rs` and should NOT have dead_code annotations:

| Line | Function | Called From |
|------|----------|-------------|
| 142 | `group_header_icon_style()` | mod.rs:409 |
| 148 | `selected_row_bg()` | mod.rs:433, 648, 1231 |
| 154 | `accent_bar_style()` | mod.rs:445, 661 |
| 160 | `kbd_badge_style()` | mod.rs:133 |
| 168 | `kbd_label_style()` | mod.rs:134, 281, 288, 315 |
| 174 | `kbd_accent_style()` | mod.rs:310 |
| 180 | `info_banner_bg()` | mod.rs:577, 993 |
| 186 | `info_banner_border_style()` | mod.rs:576, 992 |
| 192 | `empty_state_icon_style()` | mod.rs:834, 1071, 1157 |
| 198 | `empty_state_title_style()` | mod.rs:804, 849, 1041, 1086 |
| 206 | `empty_state_subtitle_style()` | mod.rs:861, 864, 1097, 1183 |

For each: remove the `#[allow(dead_code)]` attribute AND the associated comment (e.g., `// Used in Phase 4 tasks 03-06`).

#### Delete genuinely DEAD items (4 items)

These are truly unused and should be removed entirely:

| Line | Item | Reason |
|------|------|--------|
| 10-11 | `INDICATOR_WIDTH_OVERRIDE` constant | Never referenced |
| 38-45 | `indicator_style()` function | Never called |
| 115-118 | `readonly_indicator_style()` function | Never called |
| 121-124 | `info_border_style()` function | Explicitly deprecated ("Replaced by info_banner_border_style") |

#### Check mod.rs `settings` field (line 42)

```rust
#[allow(dead_code)] // Used in future tasks for rendering tab content
settings: &'a Settings,
```

If Task 05 (wire IconMode) runs before this, the field will be used and the annotation should be removed. If this task runs first, leave it until Task 05 is complete.

### Acceptance Criteria

1. No `#[allow(dead_code)]` on any function that is actively called from mod.rs
2. 4 genuinely dead items removed from styles.rs
3. `cargo clippy --workspace -- -D warnings` passes clean (no new dead_code warnings)
4. `cargo test -p fdemon-tui` passes

### Testing

No new tests needed. Verify existing tests still pass:

```bash
cargo test -p fdemon-tui
cargo clippy --workspace -- -D warnings
```

### Notes

- The `#[allow(dead_code)]` annotations were added during early Phase 4 wave implementation when styles were created before they were consumed. Now that all waves are complete, they should be cleaned up.
- Clippy will warn about any truly dead code once the annotations are removed — this is the desired behavior.
- If clippy reports new dead_code warnings after removing annotations, investigate whether those functions need to be kept or deleted.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/settings_panel/styles.rs` | Removed `#[allow(dead_code)]` annotations from 11 LIVE style functions, deleted 4 genuinely dead items (INDICATOR_WIDTH_OVERRIDE constant, indicator_style function, readonly_indicator_style function, info_border_style function) |

### Notable Decisions/Tradeoffs

1. **Only edited styles.rs**: As instructed, did not modify `mod.rs` (Task 05 will handle the `settings` field annotation) or `tests.rs` (other agents may be working there).
2. **Complete removal of dead code**: All 4 genuinely unused items were completely removed, including their documentation comments, to clean up the codebase.
3. **Preserved documentation**: Kept all doc comments for the 11 LIVE functions, only removing the `#[allow(dead_code)]` attributes and their associated inline comments.

### Testing Performed

- `cargo test -p fdemon-tui` - Passed (446 tests)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None identified**: All changes are cleanup-only. The removed functions were genuinely unused, and the annotation removals expose the code to proper dead_code detection going forward, which is the intended behavior.
