## Task: Mouse-Wheel Scroll in Performance Panel

**Objective**: Route mouse-wheel events inside the DevTools Performance panel to the existing `PerfScrollUp` / `PerfScrollDown` / `PerfPageUp` / `PerfPageDown` messages. Wheel routing uses the same `focused_section` model as keyboard scroll — no per-region wheel hit-testing, no new `Message` variants.

**Depends on**: —

**Estimated Time**: 0.5-0.75 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/devtools.rs`:
  - Replace `DevToolsPanel::Performance => None` (line 94) with `handle_performance_scroll(dir, mods)`.
  - Add `fn handle_performance_scroll(dir: ScrollDir, mods: KeyModSet) -> Option<Message>` mirroring `handle_inspector_scroll`:
    - Plain wheel up/down → `Message::PerfScrollUp` / `PerfScrollDown`.
    - `mods.is_shift_only()` wheel up/down → `Message::PerfPageUp` / `PerfPageDown`.
    - Ctrl / Alt / any other modifier combination → `None`.
    - Horizontal wheel (`ScrollDir::Left` / `Right`) → `None`.
  - Add unit tests in the existing `tests` module covering: plain up, plain down, shift+up, shift+down, ctrl-modifier rejected, horizontal rejected.
  - Update the file-header doc comment line 6 to reflect the new behaviour (drop "frame timeline is keyboard Left/Right only").

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/mouse/devtools.rs` — `handle_inspector_scroll` as the structural template.
- `crates/fdemon-app/src/handler/mouse/link_highlight.rs` — Shift→Page pattern.
- `crates/fdemon-app/src/handler/devtools/performance.rs` — confirm `PerfScrollUp` / `PerfScrollDown` / `PerfPageUp` / `PerfPageDown` handlers already key off `focused_section`.

### Acceptance Criteria

1. Wheel-up over the Performance panel (any section) emits `Message::PerfScrollUp`; the existing handler routes by `focused_section` and behaves identically to keyboard `↑`/`k`.
2. Wheel-down emits `PerfScrollDown`.
3. Shift+wheel-up/down emits `PerfPageUp`/`PerfPageDown`.
4. Modifier combinations involving Ctrl or Alt return `None`.
5. Horizontal wheel returns `None`.
6. Filter-input / other modal gates do **not** apply (Performance has no filter mode).
7. `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
8. Manual smoke (documented in completion summary): scroll wheel over frame chart, memory chart, and alloc table; verify each scrolls its focused section.

### Notes

- The wheel landing position (x, y) is intentionally ignored. The keyboard-scroll dispatch already routes by `focused_section`; matching that behaviour avoids the "wheel-over-unfocused-section silently does nothing" UX trap and keeps the handler trivially testable.
- A future enhancement could focus-then-scroll on wheel — i.e., set `focused_section` to the section under the cursor before dispatching the scroll. Out of scope here; can be a separate task once we have user feedback on whether unfocused-section wheel is confusing.
- The handler-level `PerfScrollUp`/`Down` already clamp the offset bounds; no additional clamping needed at the mouse layer.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/devtools.rs` | Updated module header doc comment; replaced `DevToolsPanel::Performance => None` with `handle_performance_scroll(dir, mods)`; added `handle_performance_scroll` function; replaced `performance_wheel_is_always_none` test with 7 new targeted tests |
| `crates/fdemon-app/src/handler/tests.rs` | Updated `mouse_scroll_devtools_performance_shift_up_produces_none` test (renamed to `_produces_perf_page_up`) to reflect the new behavior |

### Notable Decisions/Tradeoffs

1. **Pre-existing test update**: `tests.rs` contained an integration-level test (`mouse_scroll_devtools_performance_shift_up_produces_none`) that asserted Shift+wheel on Performance returns `None`. That expectation was correct before this task — now Shift+wheel returns `PerfPageUp`. Updated the test name and assertion to reflect the new behavior rather than deleting it, so the integration-level smoke remains.

2. **Shift-only detection**: Used `mods.is_shift_only()` (mirrors `handle_network_scroll`) so that Shift+Ctrl and Shift+Alt correctly fall through to the `mods.ctrl || mods.alt` rejection branch rather than triggering page step.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2257 unit tests, 80 integration tests passing)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Smoke Test (Manual — documented per acceptance criteria)

Manual verification requires a running Flutter session with DevTools connected. The implementation is structurally identical to `handle_network_scroll` (which has been verified in production): plain wheel up/down dispatches `PerfScrollUp`/`PerfScrollDown`, Shift+wheel dispatches `PerfPageUp`/`PerfPageDown`, and the existing keyboard handler routes each message by `focused_section`. Since no `focused_section` logic was changed, the wheel path is identical to the keyboard path and inherits its correctness.

### Risks/Limitations

1. **No manual smoke confirmation**: Cannot run a live Flutter session in the CI/worktree environment; smoke is deferred to the reviewer on a real device.
