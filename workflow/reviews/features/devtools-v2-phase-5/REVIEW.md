# Code Review: DevTools v2 Phase 5

**Date:** 2026-02-22
**Reviewer:** Claude Code (Architecture, Quality, Logic, Risk agents)
**Change Type:** Feature Implementation
**Verdict:** NEEDS WORK

---

## Summary

Phase 5 delivers network config options, allocation sort toggle, network filter input mode, small terminal rendering polish, and documentation updates across ~2,249 lines in 39 files. The implementation is architecturally sound (no layer violations, TEA pattern compliant) and well-tested. However, **3 blocking issues** must be resolved before merge.

---

## Blocking Issues

### 1. Settings navigation count blocks 10 items (CRITICAL)

**File:** `crates/fdemon-app/src/handler/settings_handlers.rs:348-352`

`get_item_count_for_tab()` returns hardcoded `17` for the Project tab, but `project_settings_items()` generates **27 items**. The comment claims `devtools (2)` but there are 8 DevTools items plus 4 DevTools Logging items the comment omits entirely.

Navigation wraps at index 16, making these items **permanently unreachable**:
- `devtools.auto_repaint_rainbow`, `devtools.auto_performance_overlay`
- All 4 DevTools Logging items (`hybrid_enabled`, `prefer_vm_level`, `show_source_indicator`, `dedupe_threshold_ms`)
- Both Editor items (`command`, `open_pattern`)

**Fix:** Replace hardcoded count with dynamic calculation:
```rust
fn get_item_count_for_tab(settings: &crate::config::Settings, tab: SettingsTab) -> usize {
    match tab {
        SettingsTab::Project => {
            crate::settings_items::project_settings_items(settings).len()
        }
        // ... similarly for other tabs
    }
}
```

Add a regression test:
```rust
#[test]
fn test_project_tab_count_matches_actual_items() {
    let settings = Settings::default();
    let count = get_item_count_for_tab(&settings, SettingsTab::Project);
    let items = crate::settings_items::project_settings_items(&settings);
    assert_eq!(count, items.len());
}
```

### 2. `default_panel` enum options stale (HIGH)

**File:** `crates/fdemon-app/src/settings_items.rs:170-188`

The `default_panel` setting offers `["inspector", "layout", "performance"]` but:
- `"layout"` was removed in Phase 2
- `"network"` was added in Phase 4 but is missing from the options list

**Fix:** Update options to `["inspector", "performance", "network"]`.

### 3. Filter bar cursor uses byte length, not display width (MAJOR)

**File:** `crates/fdemon-tui/src/widgets/devtools/network/mod.rs:270-276`

```rust
x += buffer.len() as u16;    // byte length, not char/display width
x += cursor.len() as u16;    // "█" is 3 bytes UTF-8, but 1 display column
```

For non-ASCII filter input, the cursor and hint text render at wrong positions. The cursor character `"█"` advances x by 3 instead of 1.

**Fix:** Use `buffer.chars().count() as u16` and `1_u16` for cursor advance. For full Unicode correctness, use `unicode_width::UnicodeWidthStr::width()`.

---

## Non-Blocking Concerns

### 4. Session creation API duplication (MEDIUM)

**File:** `crates/fdemon-app/src/session_manager.rs`

Four `create_session*` methods duplicate the same 6-line insertion block. The old unconfigured methods (`create_session`, `create_session_with_config`) are still public but only used in tests.

**Recommendation:** Extract a private `insert_session(session: Session) -> Result<SessionId>` helper. Consider deprecating the old methods.

### 5. `NetworkState::reset()` does not preserve configured `recording` (MEDIUM)

**File:** `crates/fdemon-app/src/session/network.rs:104-111`

`reset()` preserves `max_entries` but resets `recording` to `true` (from `Default`), ignoring `network_auto_record = false` config. Asymmetric with `max_entries` preservation.

**Recommendation:** Preserve `recording` alongside `max_entries`, or store the configured `auto_record` value for reset.

### 6. `performance` module promoted to `pub` unnecessarily (LOW)

**File:** `crates/fdemon-app/src/session/mod.rs:8`

Changed from `pub(crate) mod performance` to `pub mod performance`, but `AllocationSortColumn` is already re-exported at the `session::` level. The module itself doesn't need to be public.

**Recommendation:** Revert to `pub(crate) mod performance`.

### 7. `NetworkState::reset()` redundant field assignments (LOW)

**File:** `crates/fdemon-app/src/session/network.rs:104-111`

`filter_input_active: false` and `filter_input_buffer: String::new()` are explicitly set but match `Default` exactly. Remove to avoid implying they differ from defaults.

### 8. Asymmetric sort implementations in allocation table (LOW)

**File:** `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs:77-84`

`BySize` delegates to `profile.top_by_size()` (efficient partial sort) while `ByInstances` does a full `sort_by_key` + `truncate`. Consider adding `top_by_instances()` to `AllocationProfile` for symmetry.

### 9. Manual cell loops vs `Block::new()` idiom (NITPICK)

**Files:** Inspector `mod.rs` and DevTools `mod.rs` use manual `for y... for x...` loops to set background color, while Network `mod.rs` correctly uses `Block::new().style(...).render()`. Inconsistent.

---

## Agent Verdicts

| Agent | Verdict | Key Findings |
|-------|---------|-------------|
| Architecture Enforcer | PASS | No layer violations. TEA pattern compliant. 1 warning (pub mod visibility), 2 suggestions. |
| Code Quality Inspector | NEEDS WORK | DRY violation in session_manager (4 methods). Byte-length cursor bug. Unnecessary clone pattern. |
| Logic Reasoning Checker | CONCERNS | Settings count blocks navigation. `reset()` config loss. Filter lifecycle correct. |
| Risks/Tradeoffs Analyzer | CONCERNS | 2 blocking issues (settings count, stale enum). API footgun with old session methods. |

---

## What's Good

- **TEA compliance:** All handlers are pure state transitions returning `UpdateResult`. No I/O in update functions.
- **Key isolation:** Filter input mode's early-return block at `keys.rs:319-334` prevents all key leakthrough.
- **Test coverage:** Comprehensive — 10 filter handler tests, 8 keybinding tests, 6 render tests, 18 small terminal tests.
- **Config wiring:** Network poll interval correctly clamped to min 500ms at spawn site (`actions.rs:1473`).
- **Small terminal guards:** Reasonable thresholds with informative fallback messages.
- **Documentation:** All new public functions have doc comments. Architecture docs updated.

---

## Overall Verdict: NEEDS WORK

Fix issues #1, #2, and #3 before merge. Non-blocking concerns should be tracked for follow-up.
