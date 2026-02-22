# Action Items: DevTools v2 Phase 5

**Review Date:** 2026-02-22
**Verdict:** NEEDS WORK
**Blocking Issues:** 3

## Critical Issues (Must Fix)

### 1. Fix hardcoded settings item count

- **Source:** Logic Reasoning Checker, Risks/Tradeoffs Analyzer, Codebase Researcher
- **File:** `crates/fdemon-app/src/handler/settings_handlers.rs`
- **Line:** 348-368
- **Problem:** `get_item_count_for_tab()` returns hardcoded `17` for the Project tab, but `project_settings_items()` generates 27 items. Navigation wraps at index 16, making 10 items (DevTools Logging section, Editor section) permanently unreachable via keyboard.
- **Required Action:** Replace hardcoded value with `crate::settings_items::project_settings_items(settings).len()`. Add regression test asserting count matches actual item list length.
- **Acceptance:** All 27 Project tab items reachable by pressing j/Down repeatedly from the first item.

### 2. Fix stale `default_panel` enum options

- **Source:** Risks/Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/settings_items.rs`
- **Line:** 170-188
- **Problem:** Options list includes removed `"layout"` panel and omits valid `"network"` panel.
- **Required Action:** Change options to `["inspector", "performance", "network"]`.
- **Acceptance:** Settings UI shows correct panel options. No "layout" option visible.

### 3. Fix filter bar cursor byte-length bug

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-tui/src/widgets/devtools/network/mod.rs`
- **Line:** 270-276
- **Problem:** `buffer.len() as u16` uses byte length instead of display width. Cursor character `"█"` (3 bytes UTF-8) advances x by 3 instead of 1.
- **Required Action:** Use `buffer.chars().count() as u16` for buffer width and `1_u16` for cursor advance. Ideally use `unicode_width` crate.
- **Acceptance:** Filter bar cursor renders at correct position for ASCII and non-ASCII input.

## Major Issues (Should Fix)

### 4. Deduplicate session creation methods

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/session_manager.rs`
- **Problem:** Four `create_session*` methods duplicate the same 6-line insertion block (MAX_SESSIONS guard, insert, push order, auto-select).
- **Suggested Action:** Extract private `insert_session(session: Session) -> Result<SessionId>` helper. Consider deprecating old unconfigured methods.

### 5. Preserve `recording` config across `NetworkState::reset()`

- **Source:** Logic Reasoning Checker, Code Quality Inspector
- **File:** `crates/fdemon-app/src/session/network.rs`
- **Line:** 104-111
- **Problem:** `reset()` preserves `max_entries` but not `recording`, losing `network_auto_record = false` config on session reset.
- **Suggested Action:** Preserve `recording` alongside `max_entries` in the reset struct literal.

## Minor Issues (Consider Fixing)

### 6. Revert `pub mod performance` to `pub(crate) mod performance`
- `session/mod.rs:8` — the module doesn't need to be public since needed types are already re-exported.

### 7. Remove redundant field assignments in `NetworkState::reset()`
- `filter_input_active: false` and `filter_input_buffer: String::new()` match `Default` exactly — remove them.

### 8. Add `top_by_instances()` to `AllocationProfile` for symmetric sort
- `memory_chart/table.rs:77-84` — `BySize` uses efficient `top_by_size()` but `ByInstances` does full sort inline.

### 9. Standardize background clearing to `Block::new().style().render()` idiom
- Inspector and DevTools container use manual cell loops; Network monitor already uses the idiomatic approach.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All critical issues (#1, #2, #3) resolved
- [ ] All major issues resolved or justified
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes
- [ ] Settings page: can navigate to all Project tab items including Editor section
- [ ] Settings page: `default_panel` shows inspector/performance/network options
- [ ] Network filter bar: cursor renders correctly with ASCII input
