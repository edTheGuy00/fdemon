## Task: Fix Filter Bar Cursor Byte-Length Bug

**Objective**: Fix the network panel filter bar to use display width instead of byte length for cursor positioning, preventing incorrect rendering with the cursor character and multi-byte input.

**Depends on**: None

**Severity**: MAJOR — Cursor always 2 columns too far right (even for ASCII input)

### Scope

- `crates/fdemon-tui/Cargo.toml`: Add `unicode-width = "0.2"` as direct dependency
- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs:261-276`: Replace `.len()` with `.width()`
- `crates/fdemon-tui/src/widgets/devtools/network/tests.rs`: Add multi-byte input test

### Details

**Current code (buggy):**
```rust
// network/mod.rs:261-276
let mut x = area.x;
buf.set_string(x, area.y, prompt, prompt_style);
x += prompt.len() as u16;      // byte count — OK for ASCII "Filter: " but wrong pattern
// ...
buf.set_string(x, area.y, buffer, buffer_style);
x += buffer.len() as u16;      // byte count — wrong for multi-byte user input
// ...
buf.set_string(x, area.y, cursor, cursor_style);
x += cursor.len() as u16;      // "█" is 3 bytes but 1 display column — ALWAYS wrong
```

**Impact per line:**

| Line | String | `.len()` | `.width()` (correct) |
|------|--------|----------|---------------------|
| 265 | `"Filter: "` | 8 | 8 (coincidentally correct) |
| 270 | `"cafe"` | 5 | 4 |
| 276 | `"█"` (U+2588) | 3 | 1 |

**Fix:**
```rust
use unicode_width::UnicodeWidthStr;

x += prompt.width() as u16;
// ...
x += buffer.width() as u16;
// ...
x += cursor.width() as u16;  // "█".width() == 1
```

**Dependency:** `unicode-width 0.2.2` is already in `Cargo.lock` as a transitive dependency of `ratatui-core`. Adding it to `fdemon-tui/Cargo.toml` adds no new package.

**Alternative considered:** Refactoring to use `Paragraph`/`Line`/`Span` (matching `SearchInput` in `search_input.rs`). This would eliminate manual x tracking entirely but is a larger change. The `.width()` fix is minimal and correct. The refactor can be done separately if desired.

### Acceptance Criteria

1. `unicode-width` listed as direct dependency in `crates/fdemon-tui/Cargo.toml`
2. All three `.len() as u16` calls in `render_filter_input_bar` replaced with `.width() as u16`
3. Cursor renders at correct position for ASCII input (offset by 1, not 3)
4. At least one test with multi-byte input (CJK or Cyrillic) asserting correct cursor column
5. `cargo test -p fdemon-tui` passes
6. `cargo clippy -p fdemon-tui` passes

### Testing

Add a test following the pattern in `search_input.rs:224-248`:

```rust
#[test]
fn test_filter_input_bar_cursor_position_with_multibyte() {
    // Setup state with CJK filter input
    // "日本" is 6 bytes but 4 display columns
    state.filter_input_buffer = "日本".to_string();
    // Render and verify cursor appears at x == prompt_width + 4, not prompt_width + 6
}
```

Also verify the existing 6 ASCII tests still pass unchanged.

### Notes

- The `unicode-width` version should match what ratatui uses (`0.2`) to avoid multiple versions in the lock file
- `buf.set_string` already handles multi-byte chars correctly for rendering — only the manual `x +=` arithmetic is wrong
- The hint text position (after the cursor) is also affected since it depends on the cursor's `x` being correct

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/Cargo.toml` | Added `unicode-width = "0.2"` as direct dependency |
| `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` | Added `use unicode_width::UnicodeWidthStr;` import; replaced all three `.len() as u16` calls with `.width() as u16` in `render_filter_input_bar` |
| `crates/fdemon-tui/src/widgets/devtools/network/tests.rs` | Added `find_cursor_x` helper and three new tests: `test_filter_input_bar_cursor_position_with_multibyte` (CJK "日本"), `test_filter_input_bar_cursor_position_ascii_input`, `test_filter_input_bar_cursor_position_empty_buffer` |

### Notable Decisions/Tradeoffs

1. **No workspace-level dep entry**: `unicode-width = "0.2"` was added directly in `crates/fdemon-tui/Cargo.toml` (not as a workspace dependency) because it is only needed by `fdemon-tui`. The version `0.2` matches the transitive dep already in `Cargo.lock` (0.2.2), so no new package is introduced.
2. **`find_cursor_x` helper**: The test helper scans the buffer for any cell with the `REVERSED` modifier on row 0 (the filter bar row). This is robust because only the cursor `"█"` uses `REVERSED` in that row; it does not hard-code the cursor character itself.
3. **Minimal change**: The task described an alternative refactor to `Paragraph`/`Line`/`Span` (matching `SearchInput`) but specified the `.width()` fix as the correct minimal approach. The refactor was not performed.

### Testing Performed

- `cargo check -p fdemon-tui` — Passed
- `cargo clippy -p fdemon-tui -- -D warnings` — Passed
- `cargo test -p fdemon-tui` — Passed (757 tests: 754 existing + 3 new)
  - `test_filter_input_bar_cursor_position_with_multibyte` — Passed (cursor at col 12, not 14)
  - `test_filter_input_bar_cursor_position_ascii_input` — Passed (cursor at col 11)
  - `test_filter_input_bar_cursor_position_empty_buffer` — Passed (cursor at col 8)

### Risks/Limitations

1. **`find_cursor_x` assumes filter bar is row 0**: The test helper scans y=0 of the rendered buffer. `render_monitor` renders into `Rect::new(0, 0, w, h)`, and when `filter_input_active = true` the bar is the first rendered row, so this assumption holds for these tests.
