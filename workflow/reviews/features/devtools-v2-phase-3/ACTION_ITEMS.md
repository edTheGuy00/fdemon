# Action Items: DevTools v2 Phase 3

**Review Date:** 2026-02-21
**Verdict:** NEEDS WORK
**Blocking Issues:** 2

## Critical Issues (Must Fix)

### 1. Fix allocation table layout threshold

- **Source:** Logic & Reasoning Checker
- **File:** `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs:107`
- **Problem:** `show_table = area.height >= 9` is too high for 45% memory section minus borders. Table never shown on standard 24-row terminals.
- **Required Action:** Lower `MIN_TABLE_HEIGHT` from 3 to 2, and/or adjust the 55/45 split in `performance/mod.rs:153` to give memory more space (e.g., 50/50 or 48/52)
- **Acceptance:** Allocation table visible with at least 2 data rows on a standard 24-row terminal

### 2. Fix UTF-8 byte-slice panic in class name truncation

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs:681-684`
- **Problem:** `&class.class_name[..27]` panics on multi-byte UTF-8 class names
- **Required Action:** Replace with char-based truncation
- **Acceptance:** No panic with class names containing multi-byte Unicode characters. Add a test with a CJK or emoji class name.

## Major Issues (Should Fix)

### 3. Extract submodules to meet 500-line limit

- **Source:** Code Quality Inspector
- **Files:** `memory_chart.rs` (710 lines), `frame_chart.rs` (543 lines)
- **Suggested Action:** Extract rendering helpers to submodules (`chart_renderer.rs`, `allocation_table.rs`, `helpers.rs`)

### 4. Fix `selected_frame` stale index on buffer wrap

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/handler/update.rs:1350-1362`
- **Suggested Action:** After pushing a frame, if `selected_frame.is_some()` and the buffer was at capacity, decrement the index or clear to `None`

### 5. Deduplicate frame navigation logic

- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-app/src/handler/keys.rs:379-406`
- **Suggested Action:** Add `compute_prev_frame_index(&self) -> Option<usize>` / `compute_next_frame_index(&self) -> Option<usize>` on `PerformanceState`

## Minor Issues (Consider Fixing)

### 6. Delete duplicate test `test_parse_memory_usage`

- `crates/fdemon-daemon/src/vm_service/performance.rs:332-344`

### 7. Fix `.map().flatten()` in test helper

- `crates/fdemon-app/src/handler/devtools/performance.rs:131-137` -> use `.and_then()`

### 8. Remove or wire `allocation_sort` dead state

- `crates/fdemon-app/src/session/performance.rs:70` — field stored but never read

### 9. Add `Y_AXIS_LABEL_WIDTH` constant

- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs:209`

### 10. Fix `DEFAULT_MEMORY_SAMPLE_SIZE` visibility

- `crates/fdemon-app/src/session/performance.rs:19` — change `pub` to `pub(crate)`

### 11. Remove unnecessary `Arc` on `watch::Sender`

- `crates/fdemon-app/src/actions.rs:618`

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] Critical issue #1 resolved — allocation table visible on 24-row terminal
- [ ] Critical issue #2 resolved — no UTF-8 panic on multi-byte class names
- [ ] Major issue #3 resolved — both files under 500 lines
- [ ] Verification commands pass: `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings`
