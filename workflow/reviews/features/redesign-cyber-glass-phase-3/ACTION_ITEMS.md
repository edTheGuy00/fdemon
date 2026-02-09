# Action Items: Phase 3 - New Session Modal Redesign

**Review Date:** 2026-02-09
**Verdict:** NEEDS WORK
**Blocking Issues:** 2

## Critical Issues (Must Fix)

### 1. Add Dart Defines Field to Rendered Layout
- **Source:** All 4 reviewer agents
- **File:** `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`
- **Problem:** DartDefines exists in `LaunchContextField` navigation enum but is not rendered in any layout mode, creating a ghost navigable field
- **Required Action:** Add DartDefines `ActionField` rendering to `calculate_fields_layout()` and `render_common_fields()`, OR remove DartDefines from `LaunchContextField::next()`/`prev()` in `crates/fdemon-app/src/new_session_dialog/types.rs`
- **Acceptance:** User can see and interact with DartDefines field in horizontal layout, OR navigation skips directly from EntryPoint to Launch with no dead keypress

### 2. Implement LaunchButton Focus Visual Feedback
- **Source:** All 4 reviewer agents
- **File:** `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs:374-406`
- **Problem:** `LaunchButton::render()` ignores `is_focused` field, only branches on `is_enabled`
- **Required Action:** Add focus-based styling (e.g., `BORDER_ACTIVE` border when focused+enabled)
- **Acceptance:** Launch button visually changes when focused via keyboard navigation

## Major Issues (Should Fix)

### 3. Remove Stale `#[allow(dead_code)]` Annotations
- **Source:** Risks analyzer
- **File:** `crates/fdemon-tui/src/theme/palette.rs` lines 14, 43
- **Problem:** `SURFACE` and `GRADIENT_BLUE` are actively used but still have dead_code suppression
- **Suggested Action:** Remove annotations from SURFACE and GRADIENT_BLUE; keep on GRADIENT_INDIGO

### 4. Clean Up Commented-Out Test Assertions
- **Source:** Code quality inspector, logic checker
- **File:** `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` lines 512, 1109, 1281, 1748
- **Problem:** Commented-out DartDefines assertions with misleading comments
- **Suggested Action:** Remove commented assertions; add proper assertions after fixing issue #1

### 5. Fix Dart Defines Modal Overlay Consistency
- **Source:** Risks analyzer
- **File:** `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` line 412
- **Problem:** Uses `Clear.render()` instead of `modal_overlay::dim_background()`
- **Suggested Action:** Replace with dim_background for consistency

### 6. Verify min_height() Arithmetic
- **Source:** Logic checker
- **File:** `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` line 769
- **Problem:** Returns 21 but comment arithmetic sums to 23; may cause button clipping
- **Suggested Action:** Recalculate based on actual layout constraints (will change if issue #1 adds DartDefines)

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All critical issues resolved
- [ ] All major issues resolved or justified
- [ ] `cargo fmt --all` passes
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace --lib` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] Manual test: Tab through all fields in horizontal layout, verify DartDefines visible
- [ ] Manual test: Tab to Launch button, verify focus visual feedback
