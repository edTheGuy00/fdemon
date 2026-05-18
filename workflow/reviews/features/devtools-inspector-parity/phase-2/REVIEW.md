# Review: DevTools Inspector Parity — Phase 2

**Review Date:** 2026-05-18
**Branch:** `feat/devtools-inspector-parity`
**Diff Base:** `806b9b7..HEAD` (11 commits, ~3,956 insertions, 22 files)
**Verdict:** ⚠️ **NEEDS WORK**
**Reviewers:** architecture_enforcer, code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer

## Summary

Phase 2 ships the Render Object tab, Flex Explorer tab, and the `getProperties` VM Service extension across all four crates. The implementation is architecturally sound at the layer-boundary level (no `core → *` violations, no `tui → daemon` leaks, TEA purity preserved in renderers) and follows the established pattern from Phase 1's layout fetch.

However, the review surfaces **one user-facing visual bug** (the reason the user requested this review), **two correctness bugs** (one cosmetic, one a real interaction-state race), and **multiple design-cleanliness concerns** in the TUI code added by Wave 3 implementors. None of the issues are layer violations; they are quality-of-implementation concerns concentrated in `flex_explorer_tab.rs` and `actions/inspector/mod.rs`.

## Critical Findings

### 🔴 C1 — Vertical "MainAxis" strip is unreadable (USER-REPORTED)

- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs:430-473`
- **Source:** code_quality_inspector, risks_tradeoffs_analyzer, architecture_enforcer
- **Severity:** CRITICAL — user-flagged, first-impression UX

The right-side strip is `MAIN_AXIS_STRIP_WIDTH = 3` columns wide. The current code:

- Column 0: arrows (`▲` top, `▼` bottom) + "MainAxis" stacked vertically letter-by-letter (`M / a / i / n / A / x / i / s`)
- Column 1: alignment label (e.g. "start") stacked vertically (`s / t / a / r / t`)
- Column 2: unused

Visual result is two adjacent vertical letter-columns at the far right of the panel — `Ms / at / ia / nr / At / x / i / s`. Hard to read; user reports it looks "pushed to the far right, not properly spaced and not nice looking." Worse, longer alignment values like `spaceBetween` (12 chars) won't fit when the available content height is small, and the human eye cannot parse two interleaved vertical strings.

By contrast, `render_main_axis_strip_horizontal` (line 700) renders `◀ Main Axis (start) ▶` as a single horizontal line — readable.

**Required Fix:** Either (a) move the MainAxis label and alignment value into the outer block title alongside the cross-axis label (single-line text change, no widening required), (b) widen the strip and write the labels as horizontal rows, or (c) reduce the strip to just `▲ / ▼` arrows centred between header and footer and put the textual labels in the outer title bar.

### 🔴 C2 — Stale response race on close-details + reopen-on-different-node

- **Files:**
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:706-714` (`handle_close_details`)
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:640-696` (`handle_open_details`)
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:392-422` (`handle_inspector_properties_fetched`)
- **Source:** logic_reasoning_checker
- **Severity:** CRITICAL — reachable in normal interactive use

Reproduction:

1. User opens details on node A. `pending_properties_node_id = Some("A")`, `properties_loading = true`, background task spawned.
2. User presses Esc. `handle_close_details` clears `details_open` and `details_node_id` but leaves `pending_properties_node_id` and `properties_loading` set.
3. User navigates to node B and opens details. `handle_open_details` evaluates `need_properties && !properties_loading` — because `properties_loading` is still `true`, **no new fetch is dispatched**, `details_node_id` is now `"B"`, `pending_properties_node_id` still `"A"`.
4. Task for A finishes, emits `DevToolsInspectorPropertiesFetched { node_id: "A" }`. Stale-guard checks `pending_properties_node_id ("A") == response.node_id ("A")` — they match, response is **applied**. User sees A's properties on B's details panel.

The properties stale-guard uses **`pending_id == response_id`**, while the layout stale-guard at `:312-321` uses **`pending_id == selected_value_id()`** (current selection). The two handlers diverge on the comparison key.

**Required Fix:** Either (a) cross-check `response.node_id == state.details_node_id` in the properties handler, or (b) have `handle_close_details` clear `pending_properties_node_id` and `properties_loading` (treating the in-flight task as orphaned). Existing test `properties_fetched_discards_stale_response` does not cover this scenario — add a regression test for "close + reopen on different node".

### 🔴 C3 — `buf.area` vs `area` in "Terminal too small" fallback

- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs:91`
- **Source:** code_quality_inspector, architecture_enforcer, risks_tradeoffs_analyzer (all three)
- **Severity:** CRITICAL — one-character fix

The fallback passes `buf.area` (full terminal buffer) instead of `area` (this tab's panel rect). When terminal is resized below `MIN_FLEX_VIZ_HEIGHT` / `MIN_FLEX_VIZ_WIDTH`, the message is centered in the whole buffer instead of the tab pane. Tests don't catch this because the test harness sets `buf.area == area`.

**Required Fix:** Replace `buf.area` with `area` on line 91. One-character edit.

## Major Findings

### 🟡 M1 — Dead parameter `inspector_state` silenced with `let _ =`

- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs:170`
- **Source:** code_quality_inspector, architecture_enforcer, risks_tradeoffs_analyzer
- **Severity:** MAJOR — anti-pattern per CODE_STANDARDS

`render_flex_viz` accepts `inspector_state: &InspectorState` and immediately silences it with `let _ = inspector_state;`. Dead API surface invites incorrect future usage assumptions.

**Required Fix:** Drop the parameter from the signature (and the call site), or prefix it with `_inspector_state` at the parameter definition if it's reserved for a documented future use.

### 🟡 M2 — Stale-guard key divergence between properties and layout handlers

- **Files:**
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:312-321` (layout uses `selected_value_id()`)
  - `crates/fdemon-app/src/handler/devtools/inspector.rs:409-411` (properties uses `pending_id == response_id`)
- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer
- **Severity:** MAJOR — fragile invariant

Reachable scenario: user A opens details on node X, switches selection (but not details) to node Y mid-flight. Layout response for X is rejected (selection != X). Properties response for X is accepted (pending == X). User sees half-loaded state.

**Required Fix:** Unify on `details_node_id` for both stale guards — that's the field that actually drives the details panel render. `selected_value_id()` is the wrong key when details is open.

### 🟡 M3 — Silent error swallow in 5 `let _ = msg_tx.send(...).await` sites

- **File:** `crates/fdemon-app/src/actions/inspector/mod.rs:535, 566, 581, 602, 652`
- **Source:** code_quality_inspector
- **Severity:** MAJOR — explicit CODE_STANDARDS anti-pattern

Five send-message-on-error paths use `let _ = msg_tx.send(...).await` while the success path (line 690) and `spawn_fetch_layout_data` use `if let Err(e) = ... { tracing::error!(...) }`. Inconsistency means failures in the error-reporting path are themselves silently dropped.

**Required Fix:** Adopt the success-path pattern (`if let Err(e) = ... { tracing::error!(...) }`) at all five sites.

### 🟡 M4 — `DiagnosticsNode.name` not ANSI-sanitized at deserialize time

- **Files:**
  - `crates/fdemon-core/src/widget_tree.rs:55` (struct field, no sanitization annotation)
  - `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs:235`
  - `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs:223`
- **Source:** security_reviewer
- **Severity:** MAJOR — defense-in-depth, but threat model is narrow

`description` and `property_type` are sanitized via `deserialize_sanitized_*` helpers. `name` is rendered directly to `buf.set_string()` in both new tabs with no sanitization. Adversarial or malformed VM Service responses with ANSI sequences in property names would reach the terminal.

**Required Fix:** Add `#[serde(deserialize_with = "deserialize_sanitized_option_string")]` to `DiagnosticsNode.name`. Same pattern already exists on `property_type`. One-line change.

### 🟡 M5 — Per-RPC timeout in sub-fetch loop, doc claims "total budget"

- **Files:**
  - `crates/fdemon-app/src/actions/inspector/mod.rs:31-38` (doc comment: "total budget")
  - `crates/fdemon-app/src/actions/inspector/mod.rs:554-558, 637-641` (per-RPC implementation)
- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer
- **Severity:** MAJOR — doc/code contract mismatch + availability concern

`PROPERTIES_FETCH_TIMEOUT = 10s` is applied to each `tokio::time::timeout` separately, both for the initial RPC and each sub-fetch in the loop. With N render-object sub-properties the worst case is `(1+N) × 10s = up to several minutes` of stuck loading state. Doc says "total time budget."

**Required Fix:** Pick one:
- Wrap the entire async block in a single outer `tokio::time::timeout(PROPERTIES_FETCH_TIMEOUT, ...)` and update the doc to "total budget" (matches doc).
- Update the doc to "per-RPC timeout" + cap `render_value_ids.truncate(MAX_RENDER_SUB_FETCHES)` (e.g. 8) so worst-case wall-clock is bounded.

## Minor Findings

### 🟢 m1 — `flex_explorer_tab.rs` (1,077 lines) and `actions/inspector/mod.rs` (907 lines) exceed 500-line ceiling

- **Source:** code_quality_inspector, risks_tradeoffs_analyzer
- CODE_STANDARDS: "Files > 500 lines should be split into submodules."
- Natural splits exist (`flex_explorer_tab/{strip.rs, child_boxes.rs, mod.rs}`; `actions/inspector/{mod.rs, layout.rs, properties.rs, widget_tree.rs}` — widget_tree already split).
- Defer to Phase 3 prep, but track now before further growth.

### 🟢 m2 — Duplicated helpers across `details/` siblings

- **Source:** code_quality_inspector, logic_reasoning_checker
- `render_muted_centered` defined in `flex_explorer_tab.rs:722` and `properties_tab.rs:145` (slightly different impls).
- `truncate_to` defined in `render_object_tab.rs:108` and `properties_tab.rs:253`.
- `render_object_tab.rs:96-104` has its own copy of `filtered_and_sorted` (`partition_*`), duplicating the shared `filter_and_sort_by_level` in `details/mod.rs:50-64` that `properties_tab` uses.
- **Fix:** Promote all three to `pub(super)` in `details/mod.rs` and delete duplicates.

### 🟢 m3 — `extract_flex_child` rejects JSON float for `flex_factor`

- **File:** `crates/fdemon-daemon/src/vm_service/extensions/layout.rs:178-182`
- **Source:** logic_reasoning_checker
- `as_u64()` rejects `1.0` (float); string `"1.0"` also fails. Diverges from `extract_layout_info` at `:118-123` which uses `as_f64()`.
- **Fix:** Align to `as_f64().map(|f| f as u32)` and `s.parse::<f64>().ok()` for symmetry.

### 🟢 m4 — `extra_actions` consumption divergence

- **File:** `crates/fdemon-app/src/process.rs`
- **Source:** architecture_enforcer, risks_tradeoffs_analyzer
- `process.rs` manually chains `result.action.into_iter().chain(result.extra_actions)` instead of using the `result.actions()` helper. Two paths invite drift.
- **Fix:** Have `process.rs` use `result.actions()`. Also consider privatizing `extra_actions` field so callers must go through `actions_vec()`.

### 🟢 m5 — Layout cache not cleared on `SessionRestartCompleted`

- **Files:** `crates/fdemon-app/src/handler/update.rs:222-251`, `crates/fdemon-app/src/state.rs:472-483`
- **Source:** logic_reasoning_checker
- Properties cache cleared via `reset_details_and_groups()`; layout cache fields (`last_fetched_node_id`, `pending_node_id`, `layout`, etc.) are NOT cleared. Pre-existing asymmetry made visible by Phase 2's new parallel cache.
- **Fix:** Move layout cache invalidation into `reset_details_and_groups`.

### 🟢 m6 — `cross_axis_label` has vacuous match

- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs:744-748`
- Both `Axis::Vertical` and `Axis::Horizontal` produce `"Cross Axis"`. Dead match arm.
- **Fix:** Simplify to `let axis_name = "Cross Axis";` or remove the `direction` parameter.

### 🟢 m7 — Wrong constant in `render_horizontal_flex` size guard

- **File:** `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs:526`
- `MAIN_AXIS_STRIP_WIDTH.min(3)` used as a height guard — semantically wrong (it's the vertical strip's column count). Happens to evaluate to 3 numerically.
- **Fix:** Introduce `const MIN_HORIZONTAL_FLEX_HEIGHT: u16 = 4;` with a derivation comment.

### 🟢 m8 — `unwrap()` in test assertions and `_unused: Option<()>` param

- **Files:** `render_object_tab.rs:296, 410-412`; `properties_tab.rs:419-420`
- **Source:** code_quality_inspector
- Use `.expect("clear message")` instead of `.unwrap()` for diagnostic test failures. Drop `_unused: Option<()>` param from `sample_node`.

### 🟢 m9 — Unsanitized `DiagnosticsNode.level`, `node_type`, `style`, `value_id`

- **Source:** security_reviewer
- Defense-in-depth; not currently rendered, but future calls could leak ANSI bytes.

### 🟢 m10 — `render_properties` unbounded

- **Source:** risks_tradeoffs_analyzer
- No size cap on `inspector.render_properties` vec; accumulates from initial call + every sub-fetch.
- **Fix:** Cap at e.g. 256 entries with a logged warning if exceeded.

## Documentation Freshness

- `docs/ARCHITECTURE.md` was updated by task 10 (Wave 4) — already covers `getProperties`, new flex types, `InspectorState` cache fields, and `UpdateResult.extra_actions`. PASS.
- `docs/REVIEW_FOCUS.md` should be updated to either (a) document `extra_actions` as an approved TEA exception (analogous to the existing `Cell<usize>` exception), or (b) note that the intended canonical pattern is chain-messages and `extra_actions` is a one-off. As written, it's an unexplained new convention.

## Verdict Rationale

Per the reviewer skill's verdict matrix:

| Reviewer | Verdict |
|----------|---------|
| architecture_enforcer | CONCERNS (3 design concerns) |
| code_quality_inspector | NEEDS WORK (1 critical + multiple major) |
| logic_reasoning_checker | CONCERN (1 real race) |
| risks_tradeoffs_analyzer | Concerns (2 blocking) |
| security_reviewer | CONCERNS (2 medium) |

Multiple reviewers returned CONCERNS/NEEDS WORK → **⚠️ NEEDS WORK**.

The three critical findings (C1, C2, C3) and the five major findings (M1–M5) should be resolved before merging to main. See `ACTION_ITEMS.md` for the prioritised punch list.

## Strengths Worth Calling Out

- Layer boundary discipline preserved across all four crates; `fdemon-core` still zero-internal-deps.
- TEA renderer purity preserved — no `Cell` writes added; rendering is fully read-only.
- Stale-response guard pattern implemented for properties (even if the key choice has a race — C2).
- `filter_and_sort_by_level` correctly extracted as a shared helper (even if `render_object_tab` carries a duplicate — m2).
- Hydration drop fallback (no-handle case) dispatches `DevToolsInspectorPropertiesFetchFailed` exactly once.
- `FlexChild.name` is sanitized via explicit `strip_ansi_codes` in `extract_flex_child`.
- Tab rendering safely degrades on small terminals (zero-area guards on every render).
- Properties / layout caches correctly cleared on tree refresh and details close.
- New `getProperties` extension follows established `extensions/` module pattern; `properties.rs` correctly uses free functions, not a `WidgetInspector` struct.
- 1,112 unit tests pass on the merged branch; full quality gate clean (`fmt + check + test + clippy`).
