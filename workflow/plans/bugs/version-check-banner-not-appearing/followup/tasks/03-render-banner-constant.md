## Task: Banner Layout Constant + Terminal-Too-Short Guard Test

**Objective**: Replace the inline `1` literals in the standalone-banner layout with a named
`BANNER_ROW_HEIGHT` constant and document the `BANNER_MIN_HEIGHT` derivation (C7), and add a test
covering the "terminal too short → no banner" guard (C5).

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/render/mod.rs`: add `BANNER_ROW_HEIGHT`; use it in the banner-area /
  content-area arithmetic; add a derivation comment to `BANNER_MIN_HEIGHT`.
- `crates/fdemon-tui/src/render/tests.rs`: add the terminal-too-short guard test.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs`: `StartupNotice` / `UiMode` types (read-only via the type
  system — do NOT edit `state.rs`).

### Details

#### C7 — Named constant for the banner row height

The banner block (`render/mod.rs:200-211`) uses bare `1` literals for the row height and the
content-area shift:

```rust
let banner_area = Rect::new(area.x, area.y, area.width, 1);
...
Rect::new(area.x, area.y + 1, area.width, area.height - 1)
```

Per `docs/CODE_STANDARDS.md` Responsive Layout Principle 4 (named constants for layout thresholds),
introduce a constant and reuse it, and document how `BANNER_MIN_HEIGHT` is derived from it:

```rust
/// Height of the standalone version-check banner, in terminal rows.
const BANNER_ROW_HEIGHT: u16 = 1;

/// Minimum terminal height required to carve out the banner row and still leave
/// at least one content row. Derived from: BANNER_ROW_HEIGHT (1) + 1 minimum
/// content row = 2. Below this, the banner is skipped so the content area is
/// never zero-height.
const BANNER_MIN_HEIGHT: u16 = BANNER_ROW_HEIGHT + 1;
```

Then rewrite the banner/content arithmetic in terms of the constant:

```rust
let banner_area = Rect::new(area.x, area.y, area.width, BANNER_ROW_HEIGHT);
let buf = frame.buffer_mut();
render_banner(notice, banner_area, buf);
Rect::new(
    area.x,
    area.y + BANNER_ROW_HEIGHT,
    area.width,
    area.height - BANNER_ROW_HEIGHT,
)
```

Keep the `should_render_banner_outside_dialog(..) && area.height >= BANNER_MIN_HEIGHT` guard exactly
as-is (the guard already prevents the `area.height - BANNER_ROW_HEIGHT` underflow).

#### C5 — Terminal-too-short guard test

The `area.height < BANNER_MIN_HEIGHT` guard (`render/mod.rs:201`) has no test. A refactor that drops
the guard would silently produce a zero-height content rect. Add a test (alongside the existing
banner render tests in `render/tests.rs`) that sets a notice, renders at a 1-row height, and asserts
no banner text appears — i.e., the guard fell back to `content_area = area`.

### Acceptance Criteria

1. `BANNER_ROW_HEIGHT` exists and is used for the banner `Rect` height, the `area.y +` offset, and
   the `area.height -` shrink — no bare `1` literals remain in the banner layout block.
2. `BANNER_MIN_HEIGHT` is expressed/derived from `BANNER_ROW_HEIGHT` and carries a derivation
   comment.
3. A new test renders with `startup_notice = Some(..)` at a 1-row terminal height and asserts the
   banner text ("New version available") is absent (guard skips the banner; no panic).
4. The existing banner tests (renders-on-normal, renders-on-loading, absent-when-none,
   no-double-render-in-dialog) still pass.
5. `cargo test -p fdemon-tui` green; `cargo clippy -p fdemon-tui --all-targets -- -D warnings`
   clean; `cargo fmt --all -- --check` clean.

### Testing

```rust
#[test]
fn banner_not_rendered_when_terminal_too_short() {
    // 1 row is below BANNER_MIN_HEIGHT (2): the guard must skip the banner.
    let mut state = /* AppState with startup_notice = Some(NewVersionAvailable { "0.5.7" }), UiMode::Normal */;
    let backend = ratatui::backend::TestBackend::new(80, 1);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| view(f, &mut state)).unwrap();
    let buffer = terminal.backend().buffer();
    let text: String = /* concatenate buffer cell contents */;
    assert!(!text.contains("New version available"));
}
```

> Match the existing tests' construction helpers (TestBackend size, `view` invocation, and the
> buffer-to-string approach used by `no_double_render_in_dialog`). Reuse them rather than inventing a
> new harness.

### Notes

- Do NOT edit `crates/fdemon-app/src/state.rs` — this task only reads `StartupNotice` / `UiMode`.
  If you believe a type change is needed, stop and flag it (it belongs to Task 02).
- Do NOT alter `should_render_banner_outside_dialog`, the dialog double-render prevention, or the
  banner copy/formatting — those are correct and validated.
- This is a pure refactor + test addition: rendering output for all valid sizes is unchanged.

---

## Completion Summary

**Status:** _pending_
