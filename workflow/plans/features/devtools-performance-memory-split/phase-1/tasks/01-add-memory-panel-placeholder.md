## Task: Add `DevToolsPanel::Memory` Variant + Placeholder Panel + `m` Shortcut

**Objective**: Introduce the `Memory` sub-tab in the DevTools tab bar with a placeholder render body. The user can press `m` (or click the tab) to switch to it, but the panel itself just renders a centred "Memory panel — coming in next step" message. This task is the structural foundation — every enum match, tab list, and footer-hint match adds the new arm. State extraction and real widget rendering arrive in T02/T03.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs` — add `Memory` variant to `DevToolsPanel` enum between `Performance` and `Network`.
- `crates/fdemon-app/src/handler/devtools/mod.rs` — add `Memory` arm to every `DevToolsPanel` match (`handle_switch_panel`, `handle_enter_devtools_mode` guard, `parse_default_panel`).
- `crates/fdemon-app/src/handler/keys.rs` — bind `InputKey::Char('m')` → `Message::SwitchDevToolsPanel(DevToolsPanel::Memory)` in the DevTools letter-shortcut block (around line 559).
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — add `Memory` to the `tabs` array, the `render_impl` dispatch match, and the `render_footer` match. Update the click-region count test from 3 → 4.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` — pattern for the placeholder render body (centred paragraph similar to the disconnected state).

### Details

#### 1. `state.rs` — extend the enum

```rust
// crates/fdemon-app/src/state.rs:130
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevToolsPanel {
    #[default]
    Inspector,
    Performance,
    Memory,    // NEW
    Network,
}
```

Order is significant — `Memory` sits between `Performance` and `Network` so it appears in tab-bar position 3 of 4.

#### 2. `handler/devtools/mod.rs` — extend the dispatch match arms

Locate `handle_switch_panel` (≈ lines 412–494). It currently has three arms (`Inspector`, `Performance`, `Network`). Add a fourth `Memory` arm whose body **mirrors the existing `Performance` arm**: it should unpause `alloc_pause_tx` so allocation profile polling continues. The existing `Performance` arm body:

```rust
DevToolsPanel::Performance => {
    if let Some(ref tx) = handle.alloc_pause_tx {
        let _ = tx.send(false);
    }
    // ...any other side-effects already present...
}
```

Copy the body verbatim to a new `DevToolsPanel::Memory =>` arm.

Locate `handle_enter_devtools_mode` (≈ line 267) — there is a guard `if active_panel == DevToolsPanel::Performance`. Change to `if matches!(active_panel, DevToolsPanel::Performance | DevToolsPanel::Memory)` so the alloc poll is unpaused when entering either tab.

Locate `parse_default_panel` (≈ lines 163–168) — extend the match to recognise `"memory"` (case-insensitive) → `DevToolsPanel::Memory`. The string lookup table is used by config parsing.

#### 3. `handler/keys.rs` — bind the `m` shortcut

In `handle_key_devtools` around line 559, the letter-shortcut block is:

```rust
InputKey::Char('i') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Inspector)),
InputKey::Char('p') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Performance)),
InputKey::Char('n') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Network)),
```

Add (in source order — between `p` and `n`):

```rust
InputKey::Char('m') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Memory)),
```

**Do not** add the `in_memory` guard block or change the `'s'` (allocation sort) binding in this task — those land in T03.

#### 4. `widgets/devtools/mod.rs` — extend tabs, dispatch, footer

##### Tab list (around lines 203–207)

```rust
let tabs = [
    (DevToolsPanel::Inspector,   "[i] Inspector"),
    (DevToolsPanel::Performance, "[p] Performance"),
    (DevToolsPanel::Memory,      "[m] Memory"),    // NEW
    (DevToolsPanel::Network,     "[n] Network"),
];
```

##### Dispatch match (around lines 120–171)

After the `Performance` arm and before the `Network` arm, add:

```rust
DevToolsPanel::Memory => {
    // Placeholder body — T03 replaces with the real MemoryPanel widget.
    let msg = Line::from(Span::styled(
        "Memory panel — coming next step.",
        Style::default().fg(palette::TEXT_MUTED),
    ));
    let y = chunks[1].y + chunks[1].height.saturating_sub(1) / 2;
    let x = chunks[1].x + chunks[1].width.saturating_sub(msg.width() as u16) / 2;
    buf.set_line(x, y, &msg, chunks[1].width);
}
```

##### Footer match (around lines 347–367)

Add a `Memory` arm with a placeholder hint:

```rust
DevToolsPanel::Memory => {
    "[Esc] Logs  [i] Inspector  [p] Performance  [b] Browser"
}
```

(T03 will replace this with the full Memory keymap hint.)

#### 5. Update the click-region count test (around line 883)

The test `devtools_tab_bar_registers_three_click_regions` asserts exactly 3 `SwitchDevToolsPanel` regions. Rename it to `devtools_tab_bar_registers_four_click_regions` and update the assertion to `4`. Also update `devtools_tab_bar_regions_cover_correct_widths` (≈ line 920): the `expected_widths` array becomes `[" [i] Inspector ", " [p] Performance ", " [m] Memory ", " [n] Network "]`.

### Acceptance Criteria

1. `cargo check --workspace --all-targets` succeeds — no unmatched-arm warnings (every `DevToolsPanel` match has four arms).
2. `cargo test -p fdemon-tui devtools::tests` passes; the two updated click-region tests now expect 4 tabs.
3. Manually starting fdemon and pressing `d` shows four tabs in the tab bar: `[i] Inspector  [p] Performance  [m] Memory  [n] Network`.
4. Pressing `m` highlights the Memory tab and shows the centred placeholder message in the panel body.
5. Clicking the Memory tab in a mouse-enabled terminal switches to it.
6. Pressing `p` (then `m`, then `p` again) does not interrupt allocation profile polling — the `alloc_pause_tx.send(false)` runs on every entry into either tab.
7. The `<config>.toml` `default_panel = "memory"` setting parses correctly to `DevToolsPanel::Memory`.

### Testing

Add the following inside `crates/fdemon-tui/src/widgets/devtools/mod.rs` `mod tests`:

```rust
#[test]
fn test_devtools_view_renders_memory_panel_placeholder() {
    let state = DevToolsViewState {
        active_panel: DevToolsPanel::Memory,
        ..Default::default()
    };
    let widget = DevToolsView::new(&state, None, IconSet::default());
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);

    let text = collect_buf_text(&buf, 80, 24);
    assert!(text.contains("Memory panel"),
        "expected Memory placeholder text, got: {text:?}");
}

#[test]
fn test_tab_bar_includes_memory_tab() {
    let state = DevToolsViewState::default();
    let widget = DevToolsView::new(&state, None, IconSet::default());
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
    widget.render_tab_bar_inner(Rect::new(0, 0, 80, 3), &mut buf, None);

    let text = collect_buf_text(&buf, 80, 3);
    assert!(text.contains("Memory"), "expected Memory tab, got: {text:?}");
}
```

Add inside `crates/fdemon-app/src/handler/keys.rs` test module:

```rust
#[test]
fn key_m_switches_to_memory_panel() {
    let mut state = AppState::default();
    state.ui_mode = UiMode::DevTools;
    state.devtools_view_state.active_panel = DevToolsPanel::Inspector;

    let msg = handle_key_normal(&state, InputKey::Char('m'));
    assert!(matches!(
        msg,
        Some(Message::SwitchDevToolsPanel(DevToolsPanel::Memory))
    ));
}
```

### Notes

- The `Memory` placeholder body is intentionally trivial — it exists to make the panel switchable. T03 replaces it with the real widget call.
- **Do not** modify `session/performance.rs`, `session/memory.rs` (does not exist yet), `handler/devtools/performance.rs`, or the `widgets/devtools/performance/memory_chart/` subtree in this task. Those belong to T02 / T03.
- **Do not** introduce `MemoryState`, `MemorySection`, or any `Mem*` `Message` variants. The Memory tab in Phase-1-Task-01 has no state of its own; it just renders a static placeholder.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-af4956c09736c843f

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `Memory` variant to `DevToolsPanel` enum between `Performance` and `Network` |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Added `Memory` arm to `parse_default_panel`, extended alloc-unpause guard in `handle_enter_devtools_mode` to include `Memory`, refactored alloc-pause guard in `handle_switch_panel` for leaving either `Performance` or `Memory`, added `Memory` arm to `handle_switch_panel` dispatch match |
| `crates/fdemon-app/src/handler/keys.rs` | Added `in_memory` local variable, bound `'m'` key to `SwitchDevToolsPanel(Memory)`, added `memory_panel_key_tests` test module |
| `crates/fdemon-app/src/handler/mouse/devtools.rs` | Added `Memory` arm to `handle_scroll` match (delegates to `handle_performance_scroll` as a placeholder), updated `horizontal_wheel_no_op_in_every_panel` test to include `Memory` panel |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Added `Memory` to tabs array, added `Memory` dispatch arm with placeholder render body, added `Memory` footer hint arm, renamed count test from 3→4 and updated expected widths, added two new tests |
| `crates/fdemon-tui/src/render/tests.rs` | Updated `view_renders_expected_devtools_tab_regions_at_80x24` assertion from 3→4 |

### Notable Decisions/Tradeoffs

1. **Mouse scroll fallback**: The `Memory` arm in `handle_scroll` delegates to `handle_performance_scroll` rather than returning `None`. This ensures scroll does something reasonable (row navigation) until T03 introduces dedicated memory scroll logic.
2. **`in_memory` suppressed**: The `in_memory` local variable is declared in `handle_key_devtools` but not yet used in any match guards (task scope excludes guard blocks). A `let _ = in_memory;` statement suppresses the unused-variable warning while preserving the naming convention for T03.
3. **Alloc-pause guard refactored**: The existing `if old_panel == Performance && panel != Performance` guard in `handle_switch_panel` was refactored to a `leaving_alloc_panel` boolean covering both `Performance` and `Memory`, so switching between the two tabs does not pause/unpause allocation polling unnecessarily.

### Testing Performed

- `cargo check --workspace --all-targets` — Passed (zero warnings, zero errors)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo test --workspace --lib` — Passed (2386 + 460 + 800 + 842 + 1122 tests across 5 crates, 0 failures)
- `cargo test -p fdemon-tui devtools` — Passed (420 tests)
- `cargo test -p fdemon-app memory_panel_key_tests` — Passed (1 test)

### Risks/Limitations

1. **Placeholder render body**: The `Memory` panel renders a static centred message. No real widget or state exists until T02/T03.
2. **Scroll fallback**: The `Memory` panel reuses `handle_performance_scroll` temporarily; T03 must replace this with appropriate memory-panel scroll semantics.
