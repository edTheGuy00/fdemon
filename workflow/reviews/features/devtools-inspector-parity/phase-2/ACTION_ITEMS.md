# Action Items: DevTools Inspector Parity — Phase 2

**Review Date:** 2026-05-18
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 3 critical + 5 major

## Critical Issues (Must Fix)

### 1. Vertical "MainAxis" strip is unreadable (USER-REPORTED)

- **Source:** code_quality_inspector, risks_tradeoffs_analyzer, architecture_enforcer
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs`
- **Lines:** 430-473 (`render_main_axis_strip_vertical`), 38 (`MAIN_AXIS_STRIP_WIDTH = 3`)
- **Problem:** The 3-column right-side strip renders "MainAxis" and the alignment value as two adjacent vertical letter-columns (`M / a / i / n / A / x / i / s` and `s / t / a / r / t`), with column 2 unused. Visually it appears as a cluster of letters at the far right edge — user described it as "pushed to the far right, not properly spaced and not nice looking." Longer alignment values (`spaceBetween` = 12 chars) won't fit when content height is small.
- **Required Action:** Redesign the vertical main-axis label. Recommended approach:
  - **Option A (preferred):** Move the MainAxis label + alignment value into the outer block title alongside the cross-axis label. Current title: `" Cross Axis: stretch "`. Extend to: `" Main ↕ start │ Cross: stretch "`. Strip then only needs to carry the `▲` / `▼` arrows centred between header and footer.
  - **Option B:** Widen `MAIN_AXIS_STRIP_WIDTH` to ~10 columns and write "MainAxis" + alignment value as horizontal rows centred vertically inside the strip.
  - **Option C:** Drop the strip entirely; replace with a one-line caption above or below the children (e.g. `↓ Main Axis: start    ↔ Cross Axis: stretch`).
- **Acceptance:** Open Flex Explorer on a `Column` widget. Both axis labels must be readable left-to-right (not letter-stacked). User confirms the visual is "nice looking."

### 2. Stale response race on close-details + reopen-on-different-node

- **Source:** logic_reasoning_checker
- **Files:**
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:706-714` (`handle_close_details`)
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:640-696` (`handle_open_details`)
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:392-422` (`handle_inspector_properties_fetched`)
- **Problem:** `handle_close_details` clears `details_open` and `details_node_id` but leaves `pending_properties_node_id` and `properties_loading` untouched. When the user re-opens details on a different node before the in-flight fetch completes, `properties_loading` is still true so no new fetch dispatches, and when the stale response arrives the pending-id-vs-response-id guard matches (both point to the old node) and the response is applied to the new node's details panel.
- **Required Action:** Pick one:
  - **Option A:** In `handle_inspector_properties_fetched` (and the failed/timeout variants), add a check that `response.node_id == state.details_node_id` before applying. Discard if they differ. (Mirrors the layout handler's pattern, but uses `details_node_id` as the comparison key — see action item #4 below.)
  - **Option B:** In `handle_close_details`, clear `pending_properties_node_id = None`, `properties_loading = false`, `pending_node_id = None`, and `layout_loading = false`. The in-flight task continues but its response is discarded by the existing pending-id guard (because pending is now None).
- **Acceptance:** New regression test in `handler/devtools/inspector.rs` tests module: "open details on A → close → open details on B (while fetch for A is in flight) → simulate fetched-message for A → assert B's properties/render_properties are unchanged."

### 3. `buf.area` vs `area` in "Terminal too small" fallback

- **Source:** code_quality_inspector, architecture_enforcer, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs:91`
- **Problem:** The top-level `render()` calls `render_muted_centered(buf.area, buf, "Terminal too small for flex visualization.")`. `buf.area` is the entire terminal buffer; it should be `area` (the tab pane). When the terminal is resized below `MIN_FLEX_VIZ_HEIGHT` / `MIN_FLEX_VIZ_WIDTH`, the message lands in the wrong location.
- **Required Action:** Change `buf.area` to `area`. One-character edit.
- **Acceptance:** Visual check by resizing the terminal below the threshold while on the Flex Explorer tab — the "too small" message should appear centred in the tab pane, not in the full terminal.

## Major Issues (Should Fix)

### 4. Stale-guard key divergence between properties and layout handlers

- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer
- **Files:**
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:312-321` (layout: uses `selected_value_id()`)
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:409-411` (properties: uses `pending_id == response_id`)
- **Problem:** Two different stale-guard strategies. Reachable scenario: user opens details on X, moves tree selection to Y mid-flight. Layout response for X is rejected (selection != X), properties response for X is accepted (pending == X). User sees half-loaded details.
- **Suggested Action:** Unify on `state.details_node_id` for both stale guards — that's the field driving the details render. Convert both handlers to: `if Some(&response.node_id) != state.details_node_id.as_ref() { return; }`.
- **Acceptance:** Both handler tests demonstrate consistent guard behaviour on the same scenarios.

### 5. Dead parameter `inspector_state` silenced with `let _ =`

- **Source:** code_quality_inspector, architecture_enforcer, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs:170`
- **Problem:** `render_flex_viz(area, buf, inspector_state, layout)` accepts `inspector_state` and immediately silences it. Anti-pattern per `docs/CODE_STANDARDS.md`.
- **Suggested Action:** Drop the parameter from `render_flex_viz`'s signature and from the call site at ~line 96. The function uses only `layout` and `area`/`buf`.
- **Acceptance:** Function signature no longer includes the unused parameter; `cargo clippy --workspace --all-targets -- -D warnings` still clean.

### 6. Silent error swallow in 5 `let _ = msg_tx.send(...).await` sites

- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/actions/inspector/mod.rs` lines ~535, 566, 581, 602, 652
- **Problem:** Error-path message sends use `let _ = ...await`. Inconsistent with the success path (line 690) and `spawn_fetch_layout_data` (lines 421, 441, 468) which use `if let Err(e) = ... { tracing::error!(...) }`. If error reporting itself fails, the failure is invisible.
- **Suggested Action:** Replace all five sites with:
  ```rust
  if let Err(e) = msg_tx.send(Message::DevToolsInspectorPropertiesFetchFailed { ... }).await {
      tracing::error!("failed to dispatch properties fetch error message: {e}");
  }
  ```
- **Acceptance:** No `let _ = msg_tx` patterns remain in `actions/inspector/mod.rs`. Grep confirms zero matches.

### 7. `DiagnosticsNode.name` not ANSI-sanitized at deserialize time

- **Source:** security_reviewer
- **File:** `crates/fdemon-core/src/widget_tree.rs:55`
- **Problem:** `name: Option<String>` has no `#[serde(deserialize_with = "deserialize_sanitized_option_string")]`, but it's rendered directly to `buf.set_string()` in `properties_tab.rs:235` and `render_object_tab.rs:223`. Adversarial or malformed VM Service responses with ANSI sequences in property names would corrupt terminal state.
- **Suggested Action:** Add `#[serde(default, rename = "name", deserialize_with = "deserialize_sanitized_option_string")]` to `DiagnosticsNode.name`. Same pattern as `property_type` at lines 104-109.
- **Acceptance:** New test in `widget_tree.rs` verifies a `DiagnosticsNode` JSON with ANSI codes in the `name` field deserializes with the codes stripped.

### 8. Per-RPC timeout in sub-fetch loop, doc claims "total budget"

- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer
- **Files:**
  - `crates/fdemon-app/src/actions/inspector/mod.rs:31-38` (doc comment claims "total time budget")
  - `crates/fdemon-app/src/actions/inspector/mod.rs:554-558, 637-641` (per-RPC `tokio::time::timeout` calls)
- **Problem:** `PROPERTIES_FETCH_TIMEOUT = 10s` is applied to each RPC separately. Worst case wall-clock = `(1 + N) × 10s` where N = render-object property count. Doc contract mismatched.
- **Suggested Action:** Pick one:
  - **Tighten code to match doc:** Wrap the entire async block in a single outer `tokio::time::timeout(PROPERTIES_FETCH_TIMEOUT, async move { ... }).await` and remove per-RPC timeouts.
  - **Relax doc to match code:** Update the doc comment to "per-RPC timeout; total budget = (1+N)×PROPERTIES_FETCH_TIMEOUT" AND cap the sub-fetch loop with `render_value_ids.truncate(MAX_RENDER_SUB_FETCHES)` where the constant is e.g. 8.
- **Acceptance:** Doc and implementation agree on a single semantic. Worst-case wall-clock is bounded by a named constant.

## Minor Issues (Consider Fixing)

### 9. File-size violations
- `flex_explorer_tab.rs` (1,077 lines) and `actions/inspector/mod.rs` (907 lines) both exceed the 500-line ceiling. Defer to Phase 3 prep, but track.

### 10. Helper duplication across `details/` siblings
- `render_muted_centered`, `truncate_to`, and `render_object_tab.rs`'s private `filtered_and_sorted` (duplicate of `details/mod.rs::filter_and_sort_by_level`). Consolidate into `details/mod.rs` as `pub(super)` helpers.

### 11. `extract_flex_child` rejects JSON float for `flex_factor`
- `layout.rs:178-182` uses `as_u64()`; align with `extract_layout_info` at `:118-123` which uses `as_f64()`.

### 12. `extra_actions` consumption divergence
- `process.rs` manually chains `action + extra_actions`. Use `result.actions()` helper. Consider privatizing `extra_actions` field.

### 13. Layout cache not cleared on `SessionRestartCompleted`
- Properties cache cleared; layout cache (pre-existing) is not. Move layout-cache clear into `reset_details_and_groups`.

### 14. Vacuous match in `cross_axis_label`
- Both `Axis::Vertical` and `Axis::Horizontal` produce `"Cross Axis"`. Simplify or remove the `direction` parameter.

### 15. Wrong constant in `render_horizontal_flex` size guard
- `MAIN_AXIS_STRIP_WIDTH.min(3)` used as a height threshold. Introduce `const MIN_HORIZONTAL_FLEX_HEIGHT: u16 = 4;`.

### 16. `unwrap()` in test assertions, `_unused: Option<()>` param
- Use `.expect("...")` for diagnostic failures. Drop the dead `_unused` parameter from the test helper.

### 17. Defense-in-depth: sanitize remaining `DiagnosticsNode` string fields
- `level`, `node_type`, `style`, `value_id` — not currently rendered, but future renderers could inherit the gap.

### 18. `render_properties` vec unbounded
- Cap at e.g. 256 entries with a logged warning if exceeded (matches log-buffer pattern from REVIEW_FOCUS).

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] All 3 critical issues resolved
- [ ] All 5 major issues resolved or justified
- [ ] User visually confirms the Flex Explorer MainAxis label is readable
- [ ] Quality gate passes: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
- [ ] New regression test for the close + reopen-on-different-node race (action #2)
- [ ] Visual regression test for the "Terminal too small" fallback placement (action #3) — or at minimum, manually verify
- [ ] No `let _ = msg_tx` patterns remain in `actions/inspector/mod.rs`
- [ ] `DiagnosticsNode.name` sanitization test added in `widget_tree.rs`
