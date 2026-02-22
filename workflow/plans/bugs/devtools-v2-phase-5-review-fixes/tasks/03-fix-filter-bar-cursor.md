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

**Status:** Not Started
