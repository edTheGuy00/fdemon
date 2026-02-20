## Task: Remove DevToolsPanel::Layout Variant

**Objective**: Remove the `Layout` variant from `DevToolsPanel`, the `'l'` keybinding, the Layout sub-tab, the Layout panel rendering dispatch, and the `layout_explorer.rs` widget file. After this task, DevTools has only two tabs: Inspector and Performance.

**Depends on**: Task 02 (merge-layout-state-into-inspector)

### Scope

- `crates/fdemon-app/src/state.rs`: Remove `Layout` variant from `DevToolsPanel` enum
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Remove Layout arm from `handle_switch_panel`, update `parse_default_panel`
- `crates/fdemon-app/src/handler/keys.rs`: Remove `'l'` keybinding
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Remove Layout tab rendering, Layout panel dispatch, Layout footer hints, module declaration, re-export
- `crates/fdemon-tui/src/widgets/devtools/layout_explorer.rs`: DELETE

### Details

#### 1. Remove `Layout` variant from `DevToolsPanel` (state.rs, lines 119-130)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevToolsPanel {
    #[default]
    Inspector,
    // REMOVED: Layout,
    Performance,
}
```

The compiler will flag every remaining `DevToolsPanel::Layout` reference — fix them all.

#### 2. Update `handle_switch_panel` (handler/devtools/mod.rs, lines 141-204)

Remove the entire `DevToolsPanel::Layout` match arm (lines 162-199). After Task 02, this arm references `inspector.*` fields — removing it is safe because layout data will be auto-fetched on tree navigation (Task 06) instead of on panel switch.

#### 3. Update `parse_default_panel` (handler/devtools/mod.rs, lines 85-91)

Map `"layout"` to `Inspector` as a backward-compatible fallback for users with `default_panel = "layout"` in their config:

```rust
pub fn parse_default_panel(panel: &str) -> DevToolsPanel {
    match panel {
        "performance" => DevToolsPanel::Performance,
        _ => DevToolsPanel::Inspector,  // "layout" falls through to Inspector
    }
}
```

#### 4. Remove `'l'` keybinding (handler/keys.rs, line 325)

Delete this line from `handle_key_devtools`:

```rust
InputKey::Char('l') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Layout)),
```

#### 5. Update TUI devtools/mod.rs

**Remove module declaration and re-export (lines 8, 11):**
```rust
// REMOVE: pub mod layout_explorer;
// REMOVE: pub use layout_explorer::LayoutExplorer;
```

**Remove Layout tab from tab bar (render_tab_bar, around line 157):**

Change tabs array from:
```rust
let tabs = [
    (DevToolsPanel::Inspector, "[i] Inspector"),
    (DevToolsPanel::Layout, "[l] Layout"),
    (DevToolsPanel::Performance, "[p] Performance"),
];
```
To:
```rust
let tabs = [
    (DevToolsPanel::Inspector, "[i] Inspector"),
    (DevToolsPanel::Performance, "[p] Performance"),
];
```

**Remove Layout panel dispatch (around lines 95-111):**

Delete the `DevToolsPanel::Layout => { ... }` match arm that creates and renders `LayoutExplorer`.

**Update footer hints (render_footer, around lines 281-310):**

Remove the `DevToolsPanel::Layout` match arm. The Inspector hints will be updated in Task 06 to include relevant info.

#### 6. Delete `layout_explorer.rs`

Delete `crates/fdemon-tui/src/widgets/devtools/layout_explorer.rs` entirely (853 lines).

**Preserve for reference**: The rendering logic in `render_layout`, `render_constraints`, `render_size_box`, and `render_flex_properties` will be reimplemented in Task 05 (`layout_panel.rs`). If helpful, read this file before deleting to understand the existing visualizations. Key elements to preserve conceptually:
- `format_constraint_value()` — format f64 with "Inf" for infinity
- Proportional size box visualization (aspect-ratio-preserving nested blocks)
- Flex properties display (flex factor, flex fit, description)

#### 7. Remove `SwitchDevToolsPanel(Layout)` from message handling

Search for any remaining match arms on `Message::SwitchDevToolsPanel` that handle `DevToolsPanel::Layout` and remove them. The top-level message dispatcher in `handler/mod.rs` or `handler/update.rs` likely has a generic `SwitchDevToolsPanel(panel)` arm that just passes through — this is fine as-is since `Layout` no longer exists as a variant.

### Acceptance Criteria

1. `DevToolsPanel` enum has exactly 2 variants: `Inspector` and `Performance`
2. `'l'` key does nothing in DevTools mode (no match arm)
3. Sub-tab bar shows only `[i] Inspector  [p] Performance`
4. No `LayoutExplorer` widget import or rendering anywhere
5. `layout_explorer.rs` file does not exist
6. `parse_default_panel("layout")` returns `DevToolsPanel::Inspector`
7. No compiler errors or warnings related to `DevToolsPanel::Layout`
8. `cargo check --workspace` passes
9. `cargo test --workspace` passes (some tests may need updating if they reference `Layout` variant)
10. `cargo clippy --workspace` clean

### Testing

```bash
cargo check --workspace   # Compiler catches all stale Layout references
cargo test --workspace    # All tests pass
```

Tests that construct `DevToolsPanel::Layout` or assert on Layout tab rendering will fail at compile time — update or remove them.

### Notes

- The `format_constraint_value` function from `layout_explorer.rs` is used by layout_panel.rs (Task 05). Either reimplement it there, or move it to a shared location in the inspector module before deleting the file.
- After this task, pressing `'l'` in DevTools mode produces `None` (unmatched key, no action). This is correct — `'l'` will not be rebound.
- The Layout panel's 14 widget tests in `layout_explorer.rs` are deleted with the file. New tests will be written in Task 05 for the replacement widget.
- The 16 devtools `mod.rs` tests may reference Layout tab rendering — update them to expect only Inspector and Performance tabs.

---

## Completion Summary

**Status:** Not started
