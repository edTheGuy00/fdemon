# Bugfix Plan: DevTools v2 Phase 5 Review Fixes

## TL;DR

Code review of DevTools v2 Phase 5 found 3 blocking bugs (hardcoded settings navigation count, stale default_panel options, filter bar cursor byte-length), 2 major issues (session creation duplication, NetworkState reset config loss), and 4 minor quality issues. This plan addresses all 9 issues across 4 crates.

## Bug Reports

### Bug 1: Settings Navigation Blocks 10+ Items (CRITICAL)

**Symptom:** In the Settings page Project tab, pressing j/Down wraps at index 16 — the bottom 10 items (DevTools Logging, Editor sections) are permanently unreachable via keyboard.

**Expected:** All 27 Project tab items navigable. LaunchConfig and VSCodeConfig tabs also navigate the correct dynamic count.

**Root Cause Analysis:**
1. `get_item_count_for_tab()` in `settings_handlers.rs:348-368` returns hardcoded integer constants
2. Project tab returns `17` but `project_settings_items()` generates **27 items** — comment says `devtools (2)` but there are now 10 DevTools items
3. LaunchConfig returns hardcoded `10` but generates `7 * N` items dynamically
4. VSCodeConfig returns hardcoded `5` but generates `6 * N` items dynamically
5. `select_next(count)` in `state.rs:558` wraps via `% count`, making items beyond the hardcoded count permanently unreachable

**Affected Files:**
- `crates/fdemon-app/src/handler/settings_handlers.rs:348-368` — hardcoded counts
- `crates/fdemon-app/src/settings_items.rs` — item generators (source of truth)
- `crates/fdemon-app/src/state.rs:556-570` — `select_next`/`select_previous` consume the count

---

### Bug 2: `default_panel` Settings Options Stale (HIGH)

**Symptom:** Settings panel shows "layout" as a valid DevTools default panel, but that panel was removed in Phase 2. "network" (added in Phase 4) is not shown as an option.

**Expected:** Options are `["inspector", "performance", "network"]`.

**Root Cause Analysis:**
1. `settings_items.rs:174-186` has a stale `options` vec: `["inspector", "layout", "performance"]`
2. The `DevToolsPanel` enum in `state.rs:119-130` has `Inspector`, `Performance`, `Network` — no `Layout`
3. `parse_default_panel()` in `handler/devtools/mod.rs:88-98` already handles `"network"` and falls back `"layout"` to `Inspector` — runtime is fine, only the UI options list is stale
4. `config/types.rs:288` doc comment also references the stale list

**Affected Files:**
- `crates/fdemon-app/src/settings_items.rs:174-186` — options list (both `.value()` and `.default()`)
- `crates/fdemon-app/src/config/types.rs:288` — doc comment

---

### Bug 3: Filter Bar Cursor Uses Byte Length (MAJOR)

**Symptom:** Network panel filter bar cursor renders at wrong position for non-ASCII input. The cursor character `"█"` (3 bytes UTF-8, 1 display column) always advances x by 3 instead of 1, even for pure ASCII input.

**Expected:** Cursor and hint text render at correct terminal column for all input.

**Root Cause Analysis:**
1. `render_filter_input_bar` in `network/mod.rs:261-276` advances `x` using `.len() as u16` (byte count)
2. `"█".len()` returns 3 but its display width is 1 — cursor is always 2 columns too far right
3. Multi-byte user input (Cyrillic, CJK) would further compound the offset
4. Other widgets (`SearchInput`) avoid this by using `Paragraph`/`Line`/`Span` — ratatui handles width internally
5. `unicode-width 0.2.2` is already a transitive dependency (via ratatui) but not a direct dep of `fdemon-tui`

**Affected Files:**
- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs:261-276` — byte-length arithmetic
- `crates/fdemon-tui/Cargo.toml` — needs `unicode-width` as direct dep

---

### Bug 4: Session Creation API Duplication (MEDIUM)

**Symptom:** Four `create_session*` methods in `session_manager.rs` duplicate an identical 6-line insertion block (MAX_SESSIONS guard, insert, push order, auto-select).

**Expected:** Single private `insert_session()` helper; `create_session_with_config` (test-only, 0 production call sites) removed or collapsed.

**Root Cause Analysis:**
1. Methods grew organically: `create_session` (bare), then `_with_config`, then `_configured` (devtools), then `_with_config_configured` (both)
2. `create_session_with_config` has exactly 1 call site — in its own test. Zero production usage.
3. The insertion block is byte-for-byte identical across all 4 methods — safe to extract

**Affected Files:**
- `crates/fdemon-app/src/session_manager.rs:44-179` — all four methods

---

### Bug 5: `NetworkState::reset()` Loses `recording` Config (MEDIUM)

**Symptom:** If `network_auto_record = false` in config, calling `reset()` silently overrides `recording` back to `true` (from `Default`). Only `max_entries` is preserved.

**Expected:** Both `max_entries` and `recording` preserved across reset.

**Root Cause Analysis:**
1. `reset()` at `network.rs:104-111` only preserves `self.max_entries`, delegates rest to `Self::default()`
2. `Default` hardcodes `recording: true`
3. `with_config()` is the only path that applies `auto_record: false` — at session creation time only
4. Method is currently dead code (no handler calls it yet), but the bug will manifest when wired in
5. Also has two redundant field assignments (`filter_input_active`, `filter_input_buffer`) matching `Default`

**Affected Files:**
- `crates/fdemon-app/src/session/network.rs:104-111` — reset method

---

## Affected Modules

| File | Issues | Changes |
|------|--------|---------|
| `crates/fdemon-app/src/handler/settings_handlers.rs` | #1 | Replace hardcoded counts with dynamic calls |
| `crates/fdemon-app/src/settings_items.rs` | #1, #2 | Fix `default_panel` options; used as source of truth for counts |
| `crates/fdemon-app/src/config/types.rs` | #2 | Fix doc comment |
| `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` | #3 | Use `unicode_width` for cursor positioning |
| `crates/fdemon-tui/Cargo.toml` | #3 | Add `unicode-width` direct dependency |
| `crates/fdemon-app/src/session_manager.rs` | #4 | Extract `insert_session()` helper, remove dead method |
| `crates/fdemon-app/src/session/network.rs` | #5 | Preserve `recording` in reset, remove redundant fields |
| `crates/fdemon-app/src/session/mod.rs` | #6 | Align module visibility |
| `crates/fdemon-core/src/performance.rs` | #8 | Add `top_by_instances()` |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs` | #8 | Use new helper |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | #9 | Replace manual cell loops |
| `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` | #9 | Replace manual cell loops |

---

## Phases

### Phase 1: Critical + Major Fixes (Bugs 1-5)

All blocking and major issues. These are independent and can be dispatched as a single wave.

**Steps:**
1. Fix hardcoded `get_item_count_for_tab()` — call item builders dynamically, add regression tests
2. Fix `default_panel` options — update to `["inspector", "performance", "network"]`
3. Fix filter bar cursor — add `unicode-width` dep, use `.width()` instead of `.len()`
4. Extract `insert_session()` helper — deduplicate 4 methods, remove dead `create_session_with_config`
5. Fix `NetworkState::reset()` — preserve `recording`, remove redundant fields

### Phase 2: Minor Quality Fixes (Issues 6-9)

Non-blocking cleanup. Can be batched into a single task.

**Steps:**
6. Align `pub(crate) mod network` visibility with `performance`
7. Add `AllocationProfile::top_by_instances()` to fdemon-core
8. Replace manual cell-loop background clears with `buf.set_style(area, style)`

---

## Edge Cases & Risks

### Settings Count — Dynamic Tabs
- **Risk:** LaunchConfig/VSCodeConfig counts depend on loaded configs which require `project_path`
- **Mitigation:** The function signature must accept `&AppState` (or `&Path` + `&Settings`) instead of just `&Settings` so dynamic tabs can load their configs. Both call sites already have access to `state.project_path`.

### Session Manager — Test Churn
- **Risk:** ~150 test call sites use `create_session` — changing its signature would cause massive churn
- **Mitigation:** Keep `create_session` signature unchanged; only extract the internal insertion block and remove the dead `create_session_with_config` method.

### Filter Bar — Alternative Refactor
- **Risk:** Using `unicode-width` directly still requires manual x tracking
- **Mitigation:** The minimal fix (`.width()`) is sufficient. A full refactor to `Paragraph`/`Line`/`Span` (matching `SearchInput`) is a larger change and deferred.

---

## Task Dependency Graph

```
Phase 1 (Wave 1 — all independent, can run in parallel)
├── 01-fix-settings-item-count
├── 02-fix-default-panel-options
├── 03-fix-filter-bar-cursor
├── 04-deduplicate-session-creation
└── 05-fix-network-state-reset

Phase 2 (Wave 2 — independent of each other, depends on Phase 1 merge)
└── 06-minor-quality-fixes (batched: visibility, sort helper, cell loops)
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] All 27 Project tab settings items reachable by pressing j/Down repeatedly
- [ ] LaunchConfig/VSCodeConfig tabs navigate correct dynamic item count
- [ ] Regression tests assert count == actual items for every tab
- [ ] `default_panel` settings shows inspector/performance/network (no layout)
- [ ] Filter bar cursor renders at correct column for ASCII and multi-byte input
- [ ] Session creation has single insertion path (no duplicated blocks)
- [ ] `NetworkState::reset()` preserves both `max_entries` and `recording`
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes

### Phase 2 Complete When:
- [ ] Both session sub-modules use `pub(crate)` visibility
- [ ] `AllocationProfile::top_by_instances()` exists and is used by table renderer
- [ ] No manual cell-loop background clears remain in devtools widgets
- [ ] `cargo test --workspace` passes

---

## Milestone Deliverable

All Phase 5 review findings resolved. Code ready for re-review and merge to develop.
