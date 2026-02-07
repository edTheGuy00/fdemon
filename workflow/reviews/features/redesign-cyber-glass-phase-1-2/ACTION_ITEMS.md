# Action Items: Cyber-Glass Redesign (Phase 1 + Phase 2)

**Review Date:** 2026-02-08
**Verdict:** NEEDS WORK
**Blocking Issues:** 3

## Critical Issues (Must Fix)

### 1. Fix multi-session header to show tabs and device info
- **Source:** Logic Checker, Risks Analyzer, User Report
- **File:** `crates/fdemon-tui/src/layout.rs:64-80`
- **File:** `crates/fdemon-tui/src/widgets/header.rs:59-89`
- **Problem:** Header is 3 rows (1 inner). Tabs need 2 inner rows. `session_count` is discarded. Device pill hidden in multi-session mode.
- **Required Action:** Make `create_with_sessions` return `Length(4)` or `Length(5)` when `session_count > 1`. Update header widget to render title + tabs in 2 inner rows. Show device info in multi-session mode (at minimum in the tabs row).
- **Acceptance:** With 2+ sessions, tabs are visible and session switching hints are shown.

### 2. Replace Nerd Font icons with safe Unicode or implement fallback
- **Source:** Risks Analyzer, User Report
- **File:** `crates/fdemon-tui/src/theme/icons.rs`
- **File:** `crates/fdemon-tui/src/widgets/header.rs:222-231`
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs:647,734,766,775,780`
- **Problem:** Nerd Font glyphs render as `?` on most terminals. No fallback mechanism.
- **Required Action:** Either (a) implement the `icon()` switching function with config, or (b) replace all Nerd Font icons in rendering paths with universally-supported Unicode (e.g., phase_indicator already uses safe chars: "●", "○", "↻", "✗"). Keep Nerd Font constants for future opt-in.
- **Acceptance:** Icons render correctly in both Ghostty and Zed integrated terminal.

### 3. Fix footer height desync in log view
- **Source:** Logic Checker
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs:1017-1042`
- **Problem:** `footer_height` is 1 even when footer isn't rendered (`inner.height <= 1`).
- **Required Action:** Change line 1019 to: `let footer_height = if has_footer && inner.height > 1 { 1 } else { 0 };`
- **Acceptance:** With very small log area, all available content lines are used.

## Major Issues (Should Fix)

### 4. Complete palette migration in status_bar/mod.rs
- **Source:** Architecture, Code Quality
- **File:** `crates/fdemon-tui/src/widgets/status_bar/mod.rs`
- **Problem:** 16 hardcoded `Color::` references remain.
- **Suggested Action:** Replace all `Color::` with palette constants, or delete the module if it's truly unused.

### 5. Complete palette migration in legacy tabs.rs code
- **Source:** Architecture, Code Quality
- **File:** `crates/fdemon-tui/src/widgets/tabs.rs:129-288`
- **Problem:** 10 hardcoded `Color::` references in `HeaderWithTabs` and legacy functions.
- **Suggested Action:** Delete legacy code if unused by render pipeline, or migrate to palette constants.

### 6. Fix palette migration in modal_overlay.rs
- **Source:** Code Quality
- **File:** `crates/fdemon-tui/src/widgets/modal_overlay.rs:98,133`
- **Problem:** `Color::DarkGray` and `Color::Black` used instead of palette constants.
- **Suggested Action:** Replace with `palette::TEXT_MUTED`/`palette::DEEPEST_BG`.

### 7. Remove dead code
- **Source:** Code Quality, Logic, Risks
- **Files:** `log_view/mod.rs:799` (`build_title`), `layout.rs` (7 dead functions), `status_bar/mod.rs` + `widgets/mod.rs:19` (unused exports)
- **Suggested Action:** Delete `build_title()`. Remove unused layout functions. Remove `StatusBar`/`StatusBarCompact` exports (or entire module). Remove `HeaderWithTabs` export.

### 8. Fix SOURCE_* palette constants
- **Source:** Code Quality
- **File:** `crates/fdemon-tui/src/theme/palette.rs:56-58`
- **Problem:** `SOURCE_APP`=Magenta but log_view uses `STATUS_GREEN`. Mismatch.
- **Suggested Action:** Update SOURCE_* to match actual usage, or have log_view use SOURCE_* constants.

### 9. Standardize width calculation in metadata bars
- **Source:** Logic Checker, Risks
- **File:** `crates/fdemon-tui/src/widgets/log_view/mod.rs:686`
- **Problem:** Uses `.len()` (bytes) not `.chars().count()` (chars) for padding calc.
- **Suggested Action:** Use consistent `.chars().count()` or ideally `unicode-width` crate.

### 10. Remove `#![allow(dead_code)]` from theme modules
- **Source:** Code Quality, Risks
- **Files:** `palette.rs:7`, `icons.rs:7`, `styles.rs:4`
- **Suggested Action:** Remove file-level suppression, add targeted `#[allow(dead_code)]` only on specific items needed for future phases.

## Minor Issues (Consider Fixing)

### 1. Unnecessary clones in header.rs
- `left_spans.clone()` and `shortcuts.clone()` at lines 138, 159
- Compute width from Vec directly without cloning

### 2. Magic numbers
- `header.rs:185` — padding `4`, extract to constant
- `log_view/mod.rs:1025` — compact threshold `60`, use `layout::MIN_FULL_STATUS_WIDTH`

### 3. Duplicate search overlay code
- `render/mod.rs:116-154` — two branches with identical overlay logic
- Extract to helper function

### 4. Fix icons.rs docstring
- References nonexistent `icon()` function
- Update to reflect current state

### 5. Deduplicate centered_rect
- `confirm_dialog.rs` has its own `centered_rect()` duplicating `modal_overlay::centered_rect()`

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All 3 critical issues resolved
- [ ] All major issues resolved or justified
- [ ] Multi-session tabs visible with 2+ sessions
- [ ] Icons render in both Ghostty and Zed terminals
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings`
- [ ] No hardcoded `Color::` in non-theme production code (except tests)
