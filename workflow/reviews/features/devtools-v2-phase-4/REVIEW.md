# Review: DevTools v2 Phase 4 — Network Monitor Tab

**Review Date:** 2026-02-21
**Feature:** DevTools v2 Phase 4 — Network Monitor Tab (9 tasks)
**Scope:** 31 files changed, ~1,232 lines added across all 4 crates + binary
**Verdict:** :warning: **NEEDS WORK**

---

## Verification Results

| Check | Status |
|-------|--------|
| `cargo check --workspace` | PASS |
| `cargo test --lib --workspace` | PASS (696 tests) |
| `cargo clippy --workspace` | PASS (0 warnings) |
| `cargo fmt --all` | PASS |
| E2E tests | 25 pre-existing failures (settings page, startup screen) — not related to this change |

---

## Agent Verdicts

| Agent | Verdict | Critical Issues | Warnings |
|-------|---------|-----------------|----------|
| Architecture Enforcer | PASS | 0 | 2 |
| Code Quality Inspector | NEEDS WORK | 2 major | 7 minor, 3 nitpick |
| Logic & Reasoning Checker | CONCERNS | 1 critical | 5 warnings, 2 notes |
| Risks & Tradeoffs Analyzer | CONCERNS | 1 critical, 1 high | 3 medium, 2 low |

---

## Executive Summary

The Network Monitor Tab implementation is architecturally sound — layer boundaries are respected, the TEA pattern is faithfully maintained, and widgets are pure renderers. The codebase adds 158 new unit tests (far exceeding the 30+ target) with thorough coverage of domain types, VM Service parsing, state management, handlers, and widget rendering.

However, **four blocking issues** were identified by multiple reviewer agents:

1. **Duplicate polling tasks** on repeated panel switches (no idempotency guard)
2. **Recording toggle is non-functional** (the polling task never checks the `recording` flag)
3. **Session close leaks network task** (missing cleanup parity with performance monitoring)
4. **`truncate()` panics on multi-byte UTF-8** in the request table widget

Additionally, the user flagged a **UX issue**: narrow terminals show a full-width detail overlay (requiring Esc to go back) instead of a vertical split like the Inspector tab uses.

---

## Blocking Issues

### 1. :red_circle: CRITICAL — Duplicate polling tasks on repeated panel switches

**Found by:** Logic & Reasoning Checker
**File:** `crates/fdemon-app/src/handler/devtools/mod.rs:170-186`

`handle_switch_panel` for `DevToolsPanel::Network` unconditionally returns `StartNetworkMonitoring` without checking if a monitoring task is already running. Pressing `n` → `i` → `n` spawns a second polling task while the first continues. The second task's `VmServiceNetworkMonitoringStarted` message overwrites the lifecycle handles, orphaning the first task permanently.

Compare to the Inspector panel which guards with `inspector.root.is_none() && !inspector.loading`.

**Required Fix:** Add `handle.network_shutdown_tx.is_none()` guard before spawning.

---

### 2. :red_circle: CRITICAL — Recording toggle is non-functional

**Found by:** Code Quality Inspector, Logic Checker, Risks Analyzer (all three)
**Files:** `crates/fdemon-app/src/actions.rs:1576-1619`, `crates/fdemon-app/src/handler/devtools/network.rs:155-165`

The handler doc comment says "The polling task checks this flag each cycle and skips polls when false." This is factually incorrect. The polling loop in `spawn_network_monitoring` polls unconditionally — it has no reference to `NetworkState::recording`. The `recording` flag is purely cosmetic: toggling it changes the UI indicator (REC/PAUSED) but does not stop polling or entry merging.

**Required Fix:** Either (a) pass a `watch::Receiver<bool>` for the recording state into the polling task, or (b) check `recording` in `handle_http_profile_received` and discard entries when false. Option (b) is simpler and stays within TEA.

---

### 3. :orange_circle: HIGH — Session close leaks network monitoring task

**Found by:** Risks & Tradeoffs Analyzer
**File:** `crates/fdemon-app/src/handler/session_lifecycle.rs`

`handle_close_current_session` cleans up `vm_shutdown_tx`, `perf_task_handle`, and `perf_shutdown_tx`, but does NOT clean up `network_task_handle` or `network_shutdown_tx`. The orphaned polling task continues running until the application exits. Compare to the `VmServiceDisconnected` handler which correctly cleans up both performance and network tasks.

**Required Fix:** Add network cleanup mirroring the performance cleanup pattern:
```rust
if let Some(h) = handle.network_task_handle.take() { h.abort(); }
if let Some(tx) = handle.network_shutdown_tx.take() { let _ = tx.send(true); }
```

---

### 4. :orange_circle: HIGH — `truncate()` panics on multi-byte UTF-8

**Found by:** Code Quality Inspector, Logic Checker, Risks Analyzer
**File:** `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs:340-346`

The `truncate` function uses byte-level slicing (`&s[..max.saturating_sub(1)]`) which panics if the slice point falls within a multi-byte UTF-8 character. The doc comment claims safety, but the reasoning is flawed. A unicode-safe `truncate_str` function already exists in the parent module (`devtools/mod.rs:322-330`) using `char_indices()`.

**Required Fix:** Replace with the existing `truncate_str` or use `char_indices()` for safe slicing.

---

## Major Issues (Should Fix)

### 5. :yellow_circle: Inconsistent HTTP method color schemes

**Found by:** Logic & Reasoning Checker
**Files:** `request_table.rs:297-308` vs `request_details.rs:504-515`

| Method | Table | Details |
|--------|-------|---------|
| POST | Blue | Yellow |
| PUT | Yellow | Cyan |
| PATCH | Yellow | Cyan |
| HEAD | Cyan | Green |
| OPTIONS | Magenta | Gray |

The same request shows different method colors depending on which panel the user is looking at.

**Fix:** Extract a single `http_method_color()` function in a shared location.

---

### 6. :yellow_circle: `FetchHttpRequestDetail` hydration failure leaves spinner stuck

**Found by:** Code Quality Inspector
**File:** `crates/fdemon-app/src/process.rs:78-93`

When hydration discards `FetchWidgetTree` or `FetchLayoutData`, the code sends a failure message to clear loading spinners. But `FetchHttpRequestDetail` has no such fallback. If the VM disconnects between the handler returning the action and hydration, `loading_detail` remains `true` permanently.

**Fix:** Add a `FetchHttpRequestDetail` branch sending `VmServiceHttpRequestDetailFailed`.

---

### 7. :yellow_circle: UX — Narrow layout uses overlay instead of vertical split

**Found by:** User (explicitly flagged), Logic Checker, Risks Analyzer
**File:** `crates/fdemon-tui/src/widgets/devtools/network/mod.rs:98-108`

On narrow terminals (< 100 columns), selecting a request replaces the table entirely with a full-width detail view. The user must press Esc to return. The Inspector tab uses a vertical split (tree top, layout bottom) for the same scenario, which keeps both panels visible.

**Fix:** Replace `render_narrow_detail` with a vertical split layout matching the Inspector pattern.

---

### 8. :yellow_circle: `selected_index` semantics inconsistency with filters

**Found by:** Code Quality Inspector, Logic Checker
**File:** `crates/fdemon-app/src/session/network.rs:88-102, 140-170`

`selected_index` is used as an index into `filtered_entries()` by `select_prev/next/selected_entry`, but the eviction loop in `merge_entries` adjusts it as if it indexes into the raw `entries` Vec. When a filter is active, these two interpretations collide and the selection silently shifts to the wrong entry.

**Fix:** Either consistently track as a raw index (translate at display time) or guard eviction adjustment when filter is active.

---

## Minor Issues

| # | Issue | File | Fix |
|---|-------|------|-----|
| 9 | Boolean passed as string to VM Service (`enabled.to_string()` instead of JSON bool) | `vm_service/network.rs:65,364,432,521` | Use `serde_json::Value::Bool(enabled)` |
| 10 | Unnecessary `.clone()` in body text helpers | `fdemon-core/src/network.rs:117-130` | Use `std::str::from_utf8(&self.request_body)` returning `&str` |
| 11 | Magic number `10` for page step | `handler/devtools/network.rs:104-113` | Define `const NETWORK_PAGE_STEP: usize = 10` |
| 12 | Magic number `18` for label column width | `request_details.rs:125` | Define `const LABEL_COL_WIDTH: u16 = 18` |
| 13 | `O(n)` eviction with `Vec::remove(0)` — should be `VecDeque` | `session/network.rs:88` | Replace `Vec` with `VecDeque` for O(1) `pop_front()` |
| 14 | `filtered_count()` allocates a full Vec just for `.len()` | `session/network.rs:126-128` | Use `.filter().count()` without collecting |
| 15 | `short_content_type` checks `"text"` before `"javascript"`/`"css"` | `request_table.rs:316-334` | Move `"javascript"` and `"css"` checks before `"text"` |
| 16 | `NetworkDetailTab` is a UI concern living in `fdemon-core` | `fdemon-core/src/network.rs:237-244` | Move to `fdemon-app/src/session/network.rs` |
| 17 | Complex `Arc<Mutex<Option<JoinHandle>>>` type leaked across module boundary | `handler/devtools/network.rs:83` | Define a type alias |
| 18 | Manual cell-by-cell background clear instead of `Clear` widget | `network/mod.rs:63-69` | Use `ratatui::widgets::Clear` |
| 19 | Duplicate client/handle function variants (~250 lines) | `vm_service/network.rs` | Consider trait abstraction |

---

## Strengths

- **Architecture compliance**: All layer boundaries respected. TEA pattern strictly followed. No violations found.
- **Test coverage**: 158 new tests across 4 crates, far exceeding the 30+ target. Covers domain types, parsing, state, handlers, and widget rendering with edge cases.
- **Documentation**: Thorough module-level docs, protocol assumptions clearly stated, all public items documented.
- **Error handling**: Defensive parsing (skip malformed entries), proper error classification (fatal vs transient), graceful degradation for release mode.
- **Incremental polling**: Correctly uses VM Service `updatedSince` for efficient data transfer.
- **VM lifecycle management**: `VmServiceDisconnected` handler properly cleans up both performance and network tasks (session close is the gap).

---

## Verdict Rationale

| Condition | Applied? |
|-----------|----------|
| Any agent returns REJECTED/FAIL | No |
| Multiple agents return CONCERNS | Yes (Logic + Risks) |
| One agent returns NEEDS WORK | Yes (Code Quality) |

**Result:** :warning: **NEEDS WORK** — Four blocking issues must be resolved before merge. The implementation is architecturally sound and well-tested, but has functional bugs (recording toggle, duplicate tasks, task leak) and a crash risk (truncate panic) that need addressing.
