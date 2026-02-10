# Action Items: Redesign Cyber-Glass Phase 4

**Review Date:** 2026-02-10
**Verdict:** :warning: NEEDS WORK
**Blocking Issues:** 2

## Critical Issues (Must Fix)

### 1. Fix info banner height allocation
- **Source:** Logic & Reasoning Checker + user bug report
- **File:** `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`
- **Lines:** 507 (USER tab), 910 (VSCODE tab)
- **Problem:** Info banners allocate `height: 3` but `Borders::ALL` consumes 2 lines, leaving inner height = 1. The guard `if inner.height < 2 { return; }` always triggers, so banners render empty.
- **Required Action:**
  1. Change `Rect::new(area.x, area.y, area.width, 3)` to `Rect::new(area.x, area.y, area.width, 4)` at both locations
  2. Update content area offset from `area.y + 3` to `area.y + 4` at both locations
  3. Verify the banner text renders correctly with the new height
- **Acceptance:** USER tab shows "Local Settings" info banner; VSCODE tab shows "VSCode Launch Configurations" info banner

### 2. Fix empty state vertical alignment
- **Source:** Logic & Reasoning Checker + user bug report
- **File:** `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`
- **Lines:** ~794-868 (launch), ~1031-1115 (vscode not found), ~1117-1201 (vscode empty)
- **Problem:** Empty states are vertically centered with left-aligned text. Previous behavior was top-aligned with horizontal centering.
- **Required Action:**
  1. Change `start_y = area.top() + area.height.saturating_sub(total_height) / 2` to `start_y = area.top() + 1` (or small fixed offset) in all 3 empty state functions
  2. Center text horizontally using `Alignment::Center` or manual centering math
- **Acceptance:** Launch and VSCode empty states show content at top of area, centered horizontally

## Major Issues (Should Fix)

### 3. Remove dead_code annotations from used style functions
- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-tui/src/widgets/settings_panel/styles.rs`
- **Problem:** 11 style functions have `#[allow(dead_code)]` annotations but are now actively called from mod.rs
- **Suggested Action:** Remove all `#[allow(dead_code)]` annotations from style functions that are referenced in mod.rs. Functions to check: `group_header_icon_style`, `selected_row_bg`, `accent_bar_style`, `kbd_badge_style`, `kbd_label_style`, `kbd_accent_style`, `info_banner_bg`, `info_banner_border_style`, `empty_state_icon_style`, `empty_state_title_style`, `empty_state_subtitle_style`

### 4. Wire up IconMode from settings
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`
- **Problem:** All `IconSet::new(IconMode::Unicode)` calls hardcode Unicode instead of reading from `self.settings`
- **Suggested Action:** Read icon mode from settings (e.g., `self.settings.ui.icons` or equivalent) and pass to `IconSet::new()`

## Minor Issues (Consider Fixing)

### 5. Accent bar cell loses SELECTED_ROW_BG
- `buf.set_line()` for the `▎` accent bar replaces the entire cell style. Consider using `buf.cell_mut()` to set only the foreground while preserving the background.

### 6. Extract shared empty state helper
- Three empty state functions share identical layout logic. A shared `render_empty_state(buf, area, icon, title, subtitle)` helper would reduce duplication.

### 7. Track mod.rs file split
- mod.rs is at ~1200 lines vs the 500-line threshold in CODE_STANDARDS.md. Plan a future split into `header.rs`, `footer.rs`, `empty_states.rs`, `content.rs` submodules.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] Critical issue #1 resolved: info banners show content on USER and VSCODE tabs
- [ ] Critical issue #2 resolved: empty states are top-aligned and horizontally centered
- [ ] Major issue #3 resolved: no unnecessary `#[allow(dead_code)]` annotations
- [ ] `cargo test --workspace --lib` passes (441+ tests)
- [ ] `cargo clippy --workspace -- -D warnings` passes clean
- [ ] `cargo fmt --all -- --check` passes
