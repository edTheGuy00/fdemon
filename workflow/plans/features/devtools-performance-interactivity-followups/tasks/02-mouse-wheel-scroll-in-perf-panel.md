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
