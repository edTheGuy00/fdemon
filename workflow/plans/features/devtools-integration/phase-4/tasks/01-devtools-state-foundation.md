## Task: DevTools State Foundation

**Objective**: Add the core state types and message variants needed for DevTools mode — `UiMode::DevTools`, `DevToolsPanel` enum, `DevToolsViewState` struct, and all new `Message` variants. This is the foundation that all other Phase 4 tasks build upon.

**Depends on**: None

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-app/src/state.rs`: Add `UiMode::DevTools`, `DevToolsPanel`, `DevToolsViewState`, `InspectorState`
- `crates/fdemon-app/src/message.rs`: Add DevTools-related `Message` variants
- `crates/fdemon-app/src/handler/mod.rs`: Add new `UpdateAction` variants for widget tree and layout data fetching
- `crates/fdemon-app/src/lib.rs`: Re-export new public types if needed

### Details

#### 1. UiMode::DevTools Variant

Add to the existing `UiMode` enum at `state.rs:17`:

```rust
pub enum UiMode {
    // ...existing variants...

    /// DevTools panel mode - replaces log view with Inspector/Layout/Performance panels
    DevTools,
}
```

This will require adding match arms in:
- `handler/keys.rs:9-18` — key dispatch (`UiMode::DevTools => handle_key_devtools(state, key)`)
- `render/mod.rs:124-230` — render dispatch (Task 06 handles this)

#### 2. DevToolsPanel Enum

```rust
/// Active sub-panel within DevTools mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevToolsPanel {
    /// Widget tree inspector with expand/collapse navigation.
    #[default]
    Inspector,

    /// Flex layout visualization for the selected widget.
    Layout,

    /// FPS, memory usage, and frame timing display.
    Performance,
}
```

#### 3. InspectorState

State for the widget inspector tree navigation:

```rust
/// State for the widget inspector tree view.
#[derive(Debug, Clone, Default)]
pub struct InspectorState {
    /// The root widget tree node (fetched on-demand via VM Service RPC).
    pub root: Option<DiagnosticsNode>,

    /// Set of expanded node IDs (value_id). Collapsed by default.
    pub expanded: HashSet<String>,

    /// Index of the currently selected visible node (0-based flat list position).
    pub selected_index: usize,

    /// Whether a tree fetch is currently in progress.
    pub loading: bool,

    /// Error message from the last failed fetch attempt.
    pub error: Option<String>,
}
```

Add navigation helpers:

```rust
impl InspectorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle expand/collapse for the node at the given value_id.
    pub fn toggle_expanded(&mut self, value_id: &str) {
        if !self.expanded.remove(value_id) {
            self.expanded.insert(value_id.to_string());
        }
    }

    /// Check if a node is expanded.
    pub fn is_expanded(&self, value_id: &str) -> bool {
        self.expanded.contains(value_id)
    }

    /// Reset state (e.g., on session change or refresh).
    pub fn reset(&mut self) {
        self.root = None;
        self.expanded.clear();
        self.selected_index = 0;
        self.loading = false;
        self.error = None;
    }

    /// Build a flat list of visible nodes based on expand/collapse state.
    /// Returns (node_ref, depth) pairs for rendering.
    pub fn visible_nodes(&self) -> Vec<(&DiagnosticsNode, usize)> {
        let Some(root) = &self.root else {
            return vec![];
        };
        let mut result = Vec::new();
        self.collect_visible(root, 0, &mut result);
        result
    }

    fn collect_visible<'a>(
        &self,
        node: &'a DiagnosticsNode,
        depth: usize,
        result: &mut Vec<(&'a DiagnosticsNode, usize)>,
    ) {
        // Skip hidden nodes
        if !node.is_visible() {
            return;
        }
        result.push((node, depth));
        if let Some(value_id) = &node.value_id {
            if self.is_expanded(value_id) {
                for child in &node.children {
                    self.collect_visible(child, depth + 1, result);
                }
            }
        }
    }
}
```

#### 4. LayoutExplorerState

```rust
/// State for the layout explorer panel.
#[derive(Debug, Clone, Default)]
pub struct LayoutExplorerState {
    /// Layout info for the currently selected widget.
    pub layout: Option<LayoutInfo>,

    /// Whether a layout fetch is in progress.
    pub loading: bool,

    /// Error from the last failed fetch.
    pub error: Option<String>,
}
```

#### 5. DevToolsViewState

Top-level state container for the entire DevTools mode:

```rust
/// Complete state for the DevTools mode UI.
#[derive(Debug, Clone, Default)]
pub struct DevToolsViewState {
    /// Currently active sub-panel.
    pub active_panel: DevToolsPanel,

    /// Widget inspector tree state.
    pub inspector: InspectorState,

    /// Layout explorer state.
    pub layout_explorer: LayoutExplorerState,

    /// Current debug overlay states (synced from VM Service).
    pub overlay_repaint_rainbow: bool,
    pub overlay_debug_paint: bool,
    pub overlay_performance: bool,
}
```

Add to `AppState` struct (at `state.rs:304`):

```rust
pub struct AppState {
    // ...existing fields...

    /// DevTools mode view state
    pub devtools_view_state: DevToolsViewState,
}
```

Initialize in `AppState::with_settings()`:

```rust
devtools_view_state: DevToolsViewState::default(),
```

Add convenience methods on `AppState`:

```rust
impl AppState {
    /// Enter DevTools mode with the default panel.
    pub fn enter_devtools_mode(&mut self) {
        self.ui_mode = UiMode::DevTools;
    }

    /// Exit DevTools mode, return to Normal.
    pub fn exit_devtools_mode(&mut self) {
        self.ui_mode = UiMode::Normal;
    }

    /// Switch the active DevTools sub-panel.
    pub fn switch_devtools_panel(&mut self, panel: DevToolsPanel) {
        self.devtools_view_state.active_panel = panel;
    }
}
```

#### 6. New Message Variants

Add to the `Message` enum in `message.rs`:

```rust
// ── DevTools Mode (Phase 4) ──────────────────────────────────────

/// Enter DevTools mode (from Normal mode via 'd' key).
EnterDevToolsMode,

/// Exit DevTools mode (return to Normal mode via Esc).
ExitDevToolsMode,

/// Switch to a specific DevTools sub-panel.
SwitchDevToolsPanel(DevToolsPanel),

/// Open Flutter DevTools in the system browser.
OpenBrowserDevTools,

/// Request a widget tree refresh from the VM Service.
RequestWidgetTree { session_id: uuid::Uuid },

/// Widget tree data received from VM Service RPC.
WidgetTreeFetched {
    session_id: uuid::Uuid,
    root: Box<DiagnosticsNode>,
},

/// Widget tree fetch failed.
WidgetTreeFetchFailed {
    session_id: uuid::Uuid,
    error: String,
},

/// Request layout data for a specific widget node.
RequestLayoutData {
    session_id: uuid::Uuid,
    node_id: String,
},

/// Layout data received from VM Service RPC.
LayoutDataFetched {
    session_id: uuid::Uuid,
    layout: Box<LayoutInfo>,
},

/// Layout data fetch failed.
LayoutDataFetchFailed {
    session_id: uuid::Uuid,
    error: String,
},

/// Toggle a debug overlay extension (repaint rainbow, debug paint, perf overlay).
ToggleDebugOverlay {
    extension: DebugOverlayKind,
},

/// Debug overlay toggle result.
DebugOverlayToggled {
    extension: DebugOverlayKind,
    enabled: bool,
},
```

Add `DebugOverlayKind` enum:

```rust
/// The three debug overlay types that can be toggled from DevTools mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugOverlayKind {
    RepaintRainbow,
    DebugPaint,
    PerformanceOverlay,
}
```

#### 7. New UpdateAction Variants

Add to `UpdateAction` in `handler/mod.rs`:

```rust
pub enum UpdateAction {
    // ...existing variants...

    /// Fetch the widget tree from the VM Service for the Inspector panel.
    FetchWidgetTree { session_id: uuid::Uuid },

    /// Fetch layout data for a specific widget node.
    FetchLayoutData {
        session_id: uuid::Uuid,
        node_id: String,
    },

    /// Toggle a debug overlay via VM Service extension call.
    ToggleOverlay {
        session_id: uuid::Uuid,
        extension: DebugOverlayKind,
    },
}
```

### Acceptance Criteria

1. `UiMode::DevTools` variant compiles and is handled in all exhaustive `match` expressions
2. `DevToolsPanel` enum has `Inspector`, `Layout`, `Performance` variants with `Default` deriving `Inspector`
3. `InspectorState` supports expand/collapse with `HashSet<String>` and builds visible node list
4. `DevToolsViewState` aggregates inspector, layout explorer, and overlay states
5. `AppState` has `devtools_view_state: DevToolsViewState` field initialized to default
6. `AppState::enter_devtools_mode()` / `exit_devtools_mode()` / `switch_devtools_panel()` work correctly
7. All new `Message` variants added and compile
8. All new `UpdateAction` variants added and compile
9. `DebugOverlayKind` enum covers all three overlay types
10. `cargo check --workspace` passes (stub match arms added for new variants)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enter_exit_devtools_mode() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Normal;
        state.enter_devtools_mode();
        assert_eq!(state.ui_mode, UiMode::DevTools);
        state.exit_devtools_mode();
        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_switch_devtools_panel() {
        let mut state = AppState::new();
        assert_eq!(state.devtools_view_state.active_panel, DevToolsPanel::Inspector);
        state.switch_devtools_panel(DevToolsPanel::Performance);
        assert_eq!(state.devtools_view_state.active_panel, DevToolsPanel::Performance);
        state.switch_devtools_panel(DevToolsPanel::Layout);
        assert_eq!(state.devtools_view_state.active_panel, DevToolsPanel::Layout);
    }

    #[test]
    fn test_inspector_state_toggle_expanded() {
        let mut inspector = InspectorState::new();
        assert!(!inspector.is_expanded("widget-1"));
        inspector.toggle_expanded("widget-1");
        assert!(inspector.is_expanded("widget-1"));
        inspector.toggle_expanded("widget-1");
        assert!(!inspector.is_expanded("widget-1"));
    }

    #[test]
    fn test_inspector_state_reset() {
        let mut inspector = InspectorState::new();
        inspector.selected_index = 5;
        inspector.expanded.insert("widget-1".to_string());
        inspector.loading = true;
        inspector.reset();
        assert_eq!(inspector.selected_index, 0);
        assert!(inspector.expanded.is_empty());
        assert!(!inspector.loading);
        assert!(inspector.root.is_none());
    }

    #[test]
    fn test_devtools_panel_default_is_inspector() {
        assert_eq!(DevToolsPanel::default(), DevToolsPanel::Inspector);
    }

    #[test]
    fn test_devtools_view_state_default() {
        let state = DevToolsViewState::default();
        assert_eq!(state.active_panel, DevToolsPanel::Inspector);
        assert!(!state.overlay_repaint_rainbow);
        assert!(!state.overlay_debug_paint);
        assert!(!state.overlay_performance);
    }
}
```

### Notes

- **`DiagnosticsNode` is in `fdemon-core/src/widget_tree.rs`** — already fully implemented with `children`, `value_id`, `is_visible()`, `display_name()`, etc.
- **`LayoutInfo` is also in `fdemon-core/src/widget_tree.rs`** — has `constraints`, `size`, `flex_factor`, `flex_fit`, `description`.
- **Box the `DiagnosticsNode` in Messages** — widget trees can be large. Using `Box<DiagnosticsNode>` avoids inflating the `Message` enum size.
- **Stub match arms**: When adding `UiMode::DevTools` to the key handler dispatch at `keys.rs:9-18`, add a temporary `UiMode::DevTools => None` arm. The actual handler is built in Task 02. Similarly, new `Message` variants in `update.rs` should get `=> UpdateResult::none()` stubs.
- **`HashSet` import**: `InspectorState` uses `std::collections::HashSet` for expanded node tracking.
- The `InspectorState::visible_nodes()` method walks the tree respecting expand/collapse state to produce a flat renderable list — this is the same approach used by tree views in VS Code and similar tools.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `UiMode::DevTools` variant, `DevToolsPanel` enum, `InspectorState` struct with `visible_nodes()`/`toggle_expanded()`/`is_expanded()`/`reset()` methods, `LayoutExplorerState` struct, `DevToolsViewState` struct, `devtools_view_state` field on `AppState`, `enter_devtools_mode()`/`exit_devtools_mode()`/`switch_devtools_panel()` helpers, `std::collections::HashSet` import, and 5 new unit tests |
| `crates/fdemon-app/src/message.rs` | Added `DebugOverlayKind` enum, imported `DevToolsPanel`/`DiagnosticsNode`/`LayoutInfo`, added 12 new `Message` variants for DevTools mode (Enter/Exit/Switch/OpenBrowser/RequestWidgetTree/WidgetTreeFetched/WidgetTreeFetchFailed/RequestLayoutData/LayoutDataFetched/LayoutDataFetchFailed/ToggleDebugOverlay/DebugOverlayToggled) |
| `crates/fdemon-app/src/handler/mod.rs` | Added 3 new `UpdateAction` variants: `FetchWidgetTree`, `FetchLayoutData`, `ToggleOverlay` using `SessionId` type |
| `crates/fdemon-app/src/handler/keys.rs` | Added stub `UiMode::DevTools => None` match arm in `handle_key()` dispatch |
| `crates/fdemon-app/src/handler/update.rs` | Added 12 stub handler arms for all new DevTools `Message` variants with basic state mutations |
| `crates/fdemon-app/src/actions.rs` | Added 3 stub `handle_action()` arms for `FetchWidgetTree`, `FetchLayoutData`, `ToggleOverlay` (logs at debug level) |
| `crates/fdemon-app/src/lib.rs` | Re-exported `DebugOverlayKind`, `DevToolsPanel`, `DevToolsViewState`, `InspectorState`, `LayoutExplorerState` |
| `crates/fdemon-tui/src/render/mod.rs` | Added stub `UiMode::DevTools => {}` match arm in render dispatch |

### Notable Decisions/Tradeoffs

1. **`SessionId` instead of `uuid::Uuid`**: The task spec used `uuid::Uuid` in the new Message/Action variants, but `uuid` is not in fdemon-app's Cargo.toml and the codebase already uses `SessionId = u64` universally. Using `SessionId` keeps consistency with the rest of the codebase without adding a dependency.

2. **Functional stubs in `update.rs`**: The DevTools `Message` handlers in `update.rs` actually update relevant state fields (e.g., `WidgetTreeFetched` stores the root node, `DebugOverlayToggled` updates the overlay flags) rather than being pure no-ops. This makes the stubs immediately useful for testing state transitions even before Task 02-06 are implemented.

3. **`std::collections::HashSet` import**: Added at the top of `state.rs` alongside the existing imports, using the fully-qualified path for `InspectorState.expanded` as specified.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --lib --workspace` - Passed (1260 unit tests across all crates: 814 fdemon-app + 446 fdemon-tui)
- `cargo clippy --workspace -- -D warnings` - Passed

New tests added (all pass):
- `state::tests::test_enter_exit_devtools_mode`
- `state::tests::test_switch_devtools_panel`
- `state::tests::test_inspector_state_toggle_expanded`
- `state::tests::test_inspector_state_reset`
- `state::tests::test_devtools_panel_default_is_inspector`
- `state::tests::test_devtools_view_state_default`

### Risks/Limitations

1. **E2e test failures are pre-existing**: 25 e2e tests fail due to TUI process spawning/timeout issues unrelated to this task. All unit tests pass.
2. **Stubs in actions.rs log at debug level**: The `FetchWidgetTree`/`FetchLayoutData`/`ToggleOverlay` stubs in `actions.rs` only emit tracing::debug! messages. Full implementation in Tasks 03-05.
