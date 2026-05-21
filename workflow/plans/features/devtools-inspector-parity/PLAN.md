# DevTools Inspector Parity

**Status:** Draft — awaiting approval
**Author:** Planner
**Owner Crates:** `fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`

---

## 1. Problem

The Inspector tab in fdemon's TUI is functional but does not match Flutter DevTools in three areas:

1. **Widget Tree rendering** — uses fixed 2-space indent per depth. Deep BlocProvider chains in real apps push later widgets so far right that they clip off-screen and the user cannot reach them (no horizontal scroll). DevTools uses an indented tree with vertical guidelines + branch ticks AND collapses long single-child chains of implementation-only widgets (`"N more widgets..."`).
2. **Details panel is minimal** — a single "Layout Explorer" pane shows size + a box-model preview + flex line. DevTools shows three tabs (Widget properties / Render object / Flex explorer) with rich per-tab content, and shows them conditionally per widget type.
3. **Navigation model is auto-show** — scrolling the tree triggers an automatic layout-data fetch and re-renders the right pane on every Up/Down. The new model: scrolling only moves the highlight; `Enter` opens a details panel with the three tabs, and `Tab` / `Shift+Tab` cycle between them.

## 2. Goals

- Widget Tree renders like DevTools' inspector v1: per-row icon, vertical guidelines, branch ticks, single-child implementation chains collapsed behind a "N more widgets" group leader that the user can expand.
- A dedicated Details view (overlay or split mode of the Inspector tab) that holds three tabs:
  - **Widget properties** — flat property list with default badges, mini layout preview box.
  - **Render object** — render object metadata + properties (creator chain, parentData, constraints, semantics, size, paint properties).
  - **Flex explorer** — ASCII representation of flex layout showing main/cross axis, children with flex factor / fit, constraints, free space.
- Tabs are shown conditionally:
  - Always: Widget properties.
  - Render object: when the selected widget has a `RenderObject` property in `getProperties` response (`propertyType == "RenderObject"`).
  - Flex explorer: when the selected widget OR its parent is `Row` / `Column` / `Flex` (mirrors DevTools' `isFlexLayout` predicate).
- Navigation: `Enter` opens Details; `Esc` closes Details back to tree-only; `Tab` / `Shift+Tab` cycle tabs while Details is open.

## 3. Non-Goals

- We are NOT replicating DevTools' graphical flex visualization with proportional rectangles and animations — the TUI version is an ASCII/box-drawing approximation.
- We are NOT adding the on-device "select widget" highlight (`setSelectionById`) — out of scope.
- We are NOT changing the existing Layout / Inspector tab routing in the DevTools top-bar (Inspector remains its own tab next to Performance / Network).

## 3.1 Confirmed Decisions

1. **Selection while Details open: frozen.** When `details_open == true`, `Up`/`Down` are no-ops; the user must press `Esc` to return to tree mode before moving the selection. Avoids redundant `getProperties` re-fetches and keeps the keymap unambiguous.
2. **"Hide implementation widgets" toggle: in scope (Phase 1).** A user-toggleable boolean (default: ON) that flips the chain-collapse rule. When ON, contiguous chains of non-local-project wrapper widgets are folded into a leader row (`+ N more widgets...`). When OFF, every widget renders on its own row. Toggle key: `Shift+H` (while in the Inspector tab). `h` lowercase remains bound to vim-style "collapse node" — see `crates/fdemon-app/src/handler/keys.rs:637`. Stored as `inspector.hide_implementation_widgets: bool` on `InspectorState`. Persisted via `.fdemon/config.toml` under `[devtools]`.
3. **Esc semantics: tiered.** In tree mode, Esc exits DevTools to Logs (current behavior). In details mode, the first Esc closes Details back to tree mode; the second Esc exits DevTools to Logs.
4. **"default" badge source: `level == "fine"`.** Matches DevTools' `DiagnosticLevel.fine` marker (`RemoteDiagnosticsNode.level`). The existing `DiagnosticsNode.level: Option<String>` field already deserializes this.
5. **Delivery cadence: 3 bundled phases.** Phase 1 = A + B (tree rendering + hide-impl toggle + details-flow scaffold with Properties tab). Phase 2 = C + D (Render object tab + Flex explorer tab). Phase 3 = E (conditional tab visibility + polish).

## 4. Background Research

### 4.1 DevTools reference (from `/Users/ed/Dev/zabin/flutter-demon/tmp/devtools`)

| Concern | DevTools location |
|---|---|
| Tree row layout, guidelines, indent painter | `packages/devtools_app/lib/src/screens/inspector/inspector_tree_controller.dart` — `InspectorRowContent`, `_RowPainter` (lines ~1285–1500). Uses `row.ticks: List<int>` + `row.lineToParent: bool` to draw vertical guides and branch ticks. |
| Chain collapse predicate | `packages/devtools_app/lib/src/shared/diagnostics/diagnostics_node.dart` — `_alwaysVisible(node)` and `inHideableGroup` (lines 626–672). A node is "always visible" if root, `isCreatedByLocalProject`, has >1 child, or has siblings. Contiguous non-visible nodes between always-visible neighbors form a "hideable group" — the first becomes a leader, the rest are subordinates. |
| Per-type icon | `packages/devtools_app/lib/src/screens/inspector/layout_explorer/ui/widgets_theme.dart` — `WidgetTheme.themeMap` (~60-entry `Map<String, WidgetTheme>`) maps widget names (e.g. `Row`, `Column`, `Container`, `Scaffold`) to icon + color. Fallback: first capital letter in a circle. |
| Tab strip | `packages/devtools_app/lib/src/screens/inspector/widget_properties/properties_view.dart` — `DetailsTable` (lines 22–131). Tabs: Widget properties always, Render object iff `renderProperties.isNotEmpty`, Flex explorer iff `selectedNode.isFlexLayout`. |
| `isFlexLayout` | `diagnostics_node.dart:487` — `widgetRuntimeType in {Row, Column, Flex}` OR `parent` is one of those. |
| `getProperties` parsing | `inspector_controller.dart:890–932` — calls `diagnostic.getProperties(group)`; any returned node with `propertyType == "RenderObject"` is split out into `renderProperties`. Then for each render object node, `getProperties` is called again to get its own properties (constraints, size, semantics, etc.). |
| `getLayoutExplorerNode` parsing | `inspector_data_models.dart:457` — `FlexLayoutProperties._buildNode()`. Response has `renderObject.properties` for axis/alignment, plus `children` each with `size`, `constraints`, `parentData{offsetX, offsetY}`, `flexFactor`, `flexFit`. |

### 4.2 fdemon current state

| Concern | Current location | Status |
|---|---|---|
| `DiagnosticsNode` | `crates/fdemon-core/src/widget_tree.rs:1–504` | Already has `created_by_local_project`, `value_id`, `children`, `properties`, `creation_location`. Sufficient for chain-collapse + tabs; need to add a `RenderObject` extraction. |
| `LayoutInfo` | same file, line 262 | Already populated by `getLayoutExplorerNode`. Need extension for flex children list. |
| Tree rendering | `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs:78–82` | Indent = `"  ".repeat(depth)`. Needs full rewrite to guideline rendering. |
| Layout panel | `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs:121–259` | Single-pane render. Will become the "Widget properties" tab content; new Render Object + Flex Explorer tabs added alongside. |
| Inspector state | `crates/fdemon-app/src/state.rs:167–267` (`InspectorState`) | Will gain `details_open: bool`, `details_tab: DetailsTab`, `properties: Vec<DiagnosticsNode>`, `render_properties: Vec<DiagnosticsNode>`, `properties_loading/error`. |
| Visible-nodes builder | `crates/fdemon-app/src/state.rs:380–407` (`visible_nodes` + `collect_visible`) | Currently flat `(node, depth)`. Must be replaced with a richer row struct that carries `ticks` + `line_to_parent` + group leader info. |
| Handler dispatch | `crates/fdemon-app/src/handler/devtools/inspector.rs` | Handles tree fetch + nav-driven layout fetch. Needs new handlers for properties fetch + details open/close + tab change. |
| Messages | `crates/fdemon-app/src/message.rs:997, 1588, 1596` | Add `DevToolsInspectorOpenDetails`, `DevToolsInspectorCloseDetails`, `DevToolsInspectorCycleTab(direction)`, `DevToolsInspectorPropertiesFetched / Failed`, `DevToolsInspectorRenderObjectPropertiesFetched / Failed`. |
| VM Service inspector calls | `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` | Currently: `getRootWidgetTree`, `getRootWidgetSummaryTree`, `getDetailsSubtree`, `getSelectedWidget`, `disposeGroup`. **Need new:** `getProperties` (for the `valueRef` of any node). |
| Key handling | `crates/fdemon-app/src/handler/keys.rs:628–642` | Up/Down/Enter/Left/Right. `Enter`/`Right` currently both mean "Expand". We must change `Enter` to "Open details" and keep `Right` as "Expand". Add `Tab` / `Shift+Tab` for tab cycling when details open, and `Esc` close path (this will be at higher level since Esc already exits to logs). |
| Footer hint | `crates/fdemon-tui/src/widgets/devtools/mod.rs:347–350` | String to update for new key set. |
| Keybindings doc | `docs/KEYBINDINGS.md:445–457` | Update inspector section. |

### 4.3 Constraints from CODE_STANDARDS

- All numeric thresholds (indent column width, tab strip height, min Details panel width) must be **named constants with derivation comments**.
- Layout decisions are space-driven (`area.height < MIN_*`), not stateful.
- New types under `fdemon-core` only — `fdemon-tui` cannot depend on `fdemon-daemon`. Render object / flex parsing primitives must therefore live in `fdemon-core/src/widget_tree.rs`.
- Files > 500 lines should be split — anticipate splitting the new details view into per-tab files.
- All `pub` items need doc comments; new threshold constants get `///` doc rationale.

## 5. High-Level Solution

### 5.1 Tree rendering model (Phase A)

Replace `visible_nodes() -> Vec<(&DiagnosticsNode, usize)>` with a new builder:

```rust
// fdemon-core/src/widget_tree.rs — new
pub struct InspectorRow<'a> {
    pub node: &'a DiagnosticsNode,
    pub depth: usize,
    /// Depth values where a vertical guideline should be drawn through this row.
    /// Computed from "ancestor still has more siblings to render below me".
    pub ticks: Vec<usize>,
    /// True if this row's connector to its parent is the L-shaped branch.
    pub line_to_parent: bool,
    /// Group-collapse info: leader carries Some(GroupInfo { count, subordinate_ids }),
    /// followers are skipped entirely (their parent leader represents them).
    pub group: RowGroup,
}

pub enum RowGroup {
    /// Standalone row, no group collapse applies.
    None,
    /// First row of a hideable chain when the chain is collapsed (N more widgets).
    Leader { hidden_count: usize },
    /// Within an expanded leader's chain — drawn at the leader's indent + 1.
    Member,
}
```

The builder lives on `InspectorState` (or a helper on `DiagnosticsNode`) and:

1. Walks `root` pre-order honoring `expanded`.
2. While walking, for each node decides whether it `is_always_visible` (port of DevTools `_alwaysVisible`):
   - root, OR `created_by_local_project`, OR `children.len() > 1`, OR has siblings.
3. Identifies contiguous chains of `!is_always_visible` nodes and folds them into a single leader row (`RowGroup::Leader { hidden_count }`).
4. The leader row stores the chain's `value_id`s on `InspectorState.expanded_groups: HashSet<String>`; when the user presses `Right` on a leader, the chain expands and all subordinates render as `Member` rows.
5. Computes `ticks` by remembering, for each ancestor at each depth, whether more siblings remain.
6. Computes `line_to_parent = true` for every child row except the very first child of an "implicit root" / always-vertical root.

The tree painter in `tree_panel.rs` is rewritten to consume `Vec<InspectorRow>` and paint:
- For each depth `d < row.depth`: column `column_of(d)` gets a vertical `│` IFF `d in row.ticks`, otherwise space.
- Branch tick: at column `column_of(row.depth - 1)`, paint `├─` for non-last child, `└─` for last child of its parent.
- Type icon: at column `column_of(row.depth)`, paint a 1-char glyph + space; mapping from `widget_runtime_type()` → glyph (`▦` Row/Col/Flex, `▣` Container, `▤` Stack, `◯` Scaffold, ▒ Padding, etc.). Default = first capital letter in `()`.
- Then description.

**Indent column width** is a named constant (e.g. `TREE_INDENT_COLS: u16 = 2` — keep current 2-cell width because terminals are narrow).

### 5.2 Details navigation model (Phase B)

Add to `InspectorState`:

```rust
pub details_open: bool,
pub details_tab: DetailsTab,                // Properties | RenderObject | FlexExplorer
pub details_node_id: Option<String>,        // value_id of node showing in details
pub properties: Vec<DiagnosticsNode>,
pub render_properties: Vec<DiagnosticsNode>,
pub properties_loading: bool,
pub properties_error: Option<DevToolsError>,
```

Key bindings (in Inspector tab):

| Key | Tree mode | Details mode |
|---|---|---|
| `Up` / `k` | move selection | — (selection frozen) |
| `Down` / `j` | move selection | — |
| `Right` / `l` | expand node | switch to next tab |
| `Left` / `h` | collapse node | switch to prev tab |
| `Enter` | **open details** | (no-op or close) |
| `Esc` | exit DevTools → Logs | close details → tree mode |
| `Tab` / `Shift+Tab` | (unbound) | cycle tab forward / back |
| `r` | refresh tree | refresh details |
| `b` | open browser | open browser |

(Esc semantics: Today Esc exits DevTools entirely. We will make Esc close Details first if Details is open; pressing Esc again exits to Logs. This is the standard DevTools "back out of modal" pattern.)

When `Enter` is pressed in tree mode:
1. Set `details_open = true`, `details_tab = Properties`, `details_node_id = selected_value_id`.
2. Dispatch a new `UpdateAction::FetchInspectorProperties { session_id, node_id }` to fetch `getProperties` for the selected widget.
3. Layout data already auto-fetches on selection, so `LayoutInfo` is usually warm — but if not, dispatch `FetchLayoutData` too.

The TUI render in details mode replaces the right-pane with a tab strip + tab content; tree mode keeps the current 50/50 horizontal split layout (no details panel).

### 5.3 Per-tab content (Phases C–D)

**Widget properties tab** — already mostly implemented in the existing `layout_panel.rs`. Move the existing box-model + size + constraints rendering into a new file `widget_properties_tab.rs` and extend it with a property table populated from the new `properties: Vec<DiagnosticsNode>`. Each property row: `name`, value, optional `default` badge (when the property's `level == "fine"` or `defaultLevel` flags it as a default; DevTools uses the `RemoteDiagnosticsNode.level == DiagnosticLevel.fine` marker).

**Render object tab** — show the RenderObject diagnostics. Populate from the `render_properties` extracted from `getProperties`. Fields surfaced (from screenshot 4):
- `renderObject` description string
- `needsCompositing`
- `creator` chain
- `parentData`
- `constraints` (already in `LayoutInfo.constraints`)
- `layer`
- `semantics node`
- `isBlockingSemanticsOfPreviouslyPaintedNodes`
- `isSemanticBoundary`
- `size`
- Plus the render-flex specific props (direction, mainAxisAlignment, etc.) when present.

All come from the JSON inside the render-object property node's child properties.

**Flex explorer tab** — render an ASCII flex diagram from `LayoutInfo` extended with a `flex_children` list. Layout extraction must be added in `fdemon-daemon/src/vm_service/extensions/layout.rs` to read the `children` array from the `getLayoutExplorerNode` response and emit a `Vec<FlexChild>` (each with `name`, `size`, `flex_factor`, `flex_fit`, `parent_data_offset`).

ASCII visualization will use Unicode box-drawing:
- Header line: cross-axis arrow + label.
- For each child: box framed in `┌─┐ │ └─┘`, label inside or above, size label, flex factor badge if non-zero.
- Side: main-axis arrow + alignment label.
- Constraints in a footer row.

Concretely:
```
┌─ Cross Axis: center ────────────────────────────────────────────┐
│ Column                                          Total Flex: 0    │
│ ┌────────────────────────────────────────────────────────────┐ ▲│
│ │  w=180  h=341                                              │M ││
│ │     [child #1] flex=0 fit=tight                            │a ││
│ │                                                            │i ││
│ ├────────────────────────────────────────────────────────────┤n ││
│ │  w=180  h=189                                              │  ││
│ │     [child #2] flex=0 fit=tight                            │A ││
│ ├────────────────────────────────────────────────────────────┤x ││
│ │  w=180  h=341                                              │i ││
│ │     [child #3] flex=0 fit=tight                            │s ││
│ └────────────────────────────────────────────────────────────┘ ▼│
│ constraints: 0 ≤ w ≤ 392, 0 ≤ h ≤ 872   size: 180 × 872          │
└──────────────────────────────────────────────────────────────────┘
```

This is a deliberate simplification — see Risks section.

### 5.4 Conditional tab visibility (Phase E)

Add helpers on `DiagnosticsNode`:

```rust
impl DiagnosticsNode {
    pub fn is_flex(&self) -> bool {
        matches!(self.widget_runtime_type(), Some("Row" | "Column" | "Flex"))
    }
    pub fn is_flex_layout(&self) -> bool { /* self.is_flex() || parent.is_flex() */ }
}
```

Tab visibility decided at render time in `widgets/devtools/inspector/details/mod.rs`:

```rust
let show_render = !state.render_properties.is_empty();
let show_flex = selected.is_flex_layout();
```

Right/Left in details mode skip hidden tabs.

## 6. Implementation Phases & File-Level Tasks

The work is bundled into **three phases**. Each phase ends with a working build + green test suite and is independently shippable.

- **Phase 1** = original "A + B" (tree rendering + hide-implementation toggle + details navigation scaffold with the Widget properties tab populated).
- **Phase 2** = original "C + D" (Render object tab via new `getProperties` VM call + Flex explorer tab via extended `getLayoutExplorerNode` parsing).
- **Phase 3** = original "E" (conditional tab visibility + polish).

The original A/B/C/D/E sub-steps below are kept as section labels under each phase so the task breakdown can be ordered the same way during implementation.

---

### Phase 1 — Tree rendering, hide-impl toggle, details scaffold

#### 1A. Tree rendering (guidelines + chain collapse + icons)

**Files modified / created:**

| Path | Change |
|---|---|
| `crates/fdemon-core/src/widget_tree.rs` | Add `InspectorRow`, `RowGroup`, `widget_runtime_type()` helper (strips generic args from `description`), `is_always_visible()`, `is_hideable_group_member()` |
| `crates/fdemon-app/src/state.rs` | Add `expanded_groups: HashSet<String>` and `hide_implementation_widgets: bool` (default `true`) to `InspectorState`. Replace `visible_nodes` with `inspector_rows() -> Vec<InspectorRow>` that performs guideline tick computation and chain folding (chain folding is skipped when `hide_implementation_widgets == false`). Update `selected_node_description`, `selected_value_id` to walk `inspector_rows` instead. Update `reset()` to clear the new fields while preserving `hide_implementation_widgets`. |
| `crates/fdemon-app/src/config/settings.rs` | Add `hide_implementation_widgets: bool` (default `true`) to the `[devtools]` settings block. Loaded at startup, written through to `InspectorState`. |
| `crates/fdemon-app/src/message.rs` | New variant: `DevToolsInspectorToggleHideImplementation`. |
| `crates/fdemon-app/src/handler/keys.rs` | Bind `Shift+H` (uppercase `H`, Inspector tab only) → `DevToolsInspectorToggleHideImplementation`. Lowercase `h` remains "collapse node". |
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | `handle_toggle_hide_implementation` flips the flag and resets selection to 0 (because the visible-row list changes shape). Persists the change back to `Settings`. |
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | `handle_inspector_navigate` switches to `inspector_rows()`; `Expand` on a group leader row sets `expanded_groups.insert(leader_id)` rather than `expanded.insert(value_id)`. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs` | Rewrite the per-row render to draw ticks/branches/icon from `InspectorRow`. Replace `expand_icon` with `glyph_for_row` that handles leader, member, expandable, leaf cases. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` | Update the `visible` variable to use `inspector_rows()`. Adjust mouse-region rect indent math (the `(*depth as u16).checked_mul(2)` site at `tree_panel.rs:142–144`). |
| `crates/fdemon-core/src/widget_tree.rs` — tests | Unit tests: chain collapse correctness across `_alwaysVisible` predicate (root / local-project / multi-child / siblings); tick computation for arbitrary trees. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs` | Snapshot tests of `Buffer` output for several trees (deep wrapper chain, multi-child branch, mixed local + non-local). |

**Acceptance:**
- Deep chain of `BlocProvider` widgets shown in user's screenshot collapses to a single `+ 12 more widgets (▶ expand)` leader directly under `MultiBlocProvider`, mirroring DevTools' screenshot 2.
- Tree no longer drifts off the right edge: max indent column equals `(max_depth_after_collapse - 1) * INDENT_COLS + 2 cells for tick/glyph`.
- Vertical guidelines / branch ticks visible.
- Pressing `h` toggles chain collapsing on/off; the toggled state persists to `.fdemon/config.toml` under `[devtools] hide_implementation_widgets`.
- Existing keyboard navigation (Up/Down/Left/Right/r/b) unchanged behaviorally; only the rendering is different.
- Mouse click regions still correctly map to the new row positions.

#### 1B. Enter-to-details + tabbed details panel scaffold (Properties tab only)

**Files modified / created:**

| Path | Change |
|---|---|
| `crates/fdemon-app/src/state.rs` | Add `details_open`, `details_tab: DetailsTab`, `details_node_id`. Add `enum DetailsTab { Properties, RenderObject, FlexExplorer }`. Helper `visible_details_tabs(node, has_render_props)`. |
| `crates/fdemon-app/src/message.rs` | New variants: `DevToolsInspectorOpenDetails`, `DevToolsInspectorCloseDetails`, `DevToolsInspectorCycleTab { forward: bool }`. |
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | Add `handle_open_details`, `handle_close_details`, `handle_cycle_tab`. `handle_open_details` snapshots `selected_value_id` into `details_node_id` and (optionally) dispatches `FetchInspectorProperties` and `FetchLayoutData`. |
| `crates/fdemon-app/src/handler/keys.rs` | Inspector Enter → `DevToolsInspectorOpenDetails`. In details mode: `Tab` / `Shift+Tab` → `DevToolsInspectorCycleTab`. `Esc` priority handler closes details first if open. |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Adjust the existing Esc-to-Logs handler so that Esc in inspector mode with `details_open == true` only closes details, not the entire DevTools view. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` | Branch on `details_open`: if open, full right pane is a new `render_details_view`; else render existing layout panel as today. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | New file. Tab strip rendering with active-tab underline + cycling. Routes content to the per-tab renderer. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` | New file. For now port the box-model + dimensions + flex line rendering currently in `layout_panel.rs` into this file. The legacy `layout_panel.rs` stays as the "tree mode" right pane (no behavior change for users who don't press Enter). |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` (footer) | Update hint string per mode: tree mode → `[Esc] Logs [↑↓] Navigate [→] Expand [←] Collapse [Enter] Details [r] Refresh [b] Browser`. Details mode → `[Esc] Close [Tab] Next Tab [r] Refresh`. |
| `docs/KEYBINDINGS.md` | Update Inspector section. |

**Acceptance:**
- Pressing `Enter` in the tree opens a tabbed details view in the right pane.
- `Tab` / `Shift+Tab` cycle tabs (only the Properties tab is populated for now; Render Object and Flex Explorer tab stubs render `"Coming soon"` text until Phase 2; Phase 3 hides them entirely for widget types that don't apply).
- `Esc` closes details first, then exits DevTools on a second press.
- Tree navigation while details is open is **frozen**: `Up`/`Down` are no-ops in details mode. The user must press `Esc` to return to tree mode before moving the selection.

---

### Phase 2 — Render object & Flex explorer tabs

#### 2A. Render object tab (new VM service call: `getProperties`)

**Files modified / created:**

| Path | Change |
|---|---|
| `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` | New constant: `pub const GET_PROPERTIES: &str = "ext.flutter.inspector.getProperties"`. |
| `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` | New method `get_properties(value_id, group)` on `WidgetInspector`. Single-shot RPC: `{ arg: value_id, objectGroup: group }`. Response is `{ result: [DiagnosticsNode, …] }`. Returns `Vec<DiagnosticsNode>`. |
| `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` | New method `fetch_properties_with_render(value_id)` — calls `get_properties` for the widget, splits out any node with `propertyType == "RenderObject"` into a separate list, then for each RenderObject node calls `get_properties(rendered_object.value_id, …)` again to fetch its sub-properties. Returns `(widget_props, render_props)`. Mirrors `InspectorController._loadPropertiesForNode` in DevTools. |
| `crates/fdemon-core/src/widget_tree.rs` | `DiagnosticsNode` already has `properties: Vec<DiagnosticsNode>`; ensure `property_type` field exists (add if missing, deserialized from `propertyType`). |
| `crates/fdemon-app/src/message.rs` | `DevToolsInspectorPropertiesFetched { session_id, widget_props, render_props }`, `DevToolsInspectorPropertiesFetchFailed`. |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | New `UpdateAction::FetchInspectorProperties { session_id, node_id, vm_handle }`. Dispatch and wiring in the engine/task spawner (same shape as the existing `FetchLayoutData`). |
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | `handle_properties_fetched` stores into `inspector.properties` + `inspector.render_properties`; `handle_properties_fetch_failed` sets `properties_error`. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs` | New file. Render a key/value property table from `render_properties`. Long values (creator chain, etc.) wrap with ellipsis or scroll via PageDown if Phase D scrolling lands. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` | Extend to show the full property list (name + value + `default` badge) below the box-model preview. |

**Acceptance:**
- Selecting a widget and pressing Enter shows two populated tabs: Widget properties (with all properties enumerated) and Render object (with renderObject metadata + nested properties).
- Container widget shows ONLY Widget properties tab (Render object tab hidden because the Container has no `RenderObject` property in its `getProperties` response — Container itself is not a render object; the wrapped DecoratedBox/Padding is).
- Widgets like `Column` show all three tabs because they DO have a `RenderFlex` render object property.
- Property values render with the "default" badge when applicable (the `level` field on the diagnostics node — `"fine"` indicates a default value in DevTools).

#### 2B. Flex explorer tab

**Files modified / created:**

| Path | Change |
|---|---|
| `crates/fdemon-core/src/widget_tree.rs` | Extend `LayoutInfo` with `children: Vec<FlexChild>`, where `FlexChild { id, name, size, constraints, flex_factor, flex_fit, parent_offset }`. Add `FlexFit` enum. |
| `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` | Extend `extract_layout_info` to walk `node.children`, parse each child's `size`/`constraints`/`parentData`/`flexFactor`/`flexFit` from the `getLayoutExplorerNode` JSON, and populate `LayoutInfo.children`. Add unit tests for the JSON shape. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs` | New file. ASCII flex diagram drawing function. Constants for box-min-height, axis-arrow chars. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | Wire the Flex tab. |

**Acceptance:**
- Selecting a `Column` and opening details shows the Flex Explorer tab.
- Children render with size labels and flex factor badges.
- Main axis / cross axis arrows show alignment + direction.
- Constraints and total flex factor footer.

---

### Phase 3 — Conditional tab visibility + polish

**Files modified / created:**

| Path | Change |
|---|---|
| `crates/fdemon-core/src/widget_tree.rs` | `is_flex_layout(node, parent_ref)` helper. (We may need to thread parent reference through `inspector_rows` so the row carries parent type; alternatively store parent runtime type into a derived `DetailsContext` struct on details open.) |
| `crates/fdemon-app/src/state.rs` | When `handle_open_details` runs, compute `details_context: DetailsContext { is_flex_layout: bool, parent_type: Option<String> }` and store on `InspectorState`. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | Use `details_context` + `render_properties.is_empty()` to filter the visible tab list. Active tab clamps when its tab is hidden. |
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | `handle_cycle_tab` skips hidden tabs when cycling. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/tests.rs` | Snapshot tests: Container (1 tab), Padding (2 tabs), Column (3 tabs), Column-child (3 tabs because parent is flex). |

**Acceptance:** Per-widget-type tab visibility matches DevTools' rules as documented in §4.1.

## 7. Risks & Trade-offs

### 7.1 ASCII Flex Explorer fidelity

DevTools' flex visualization is proportional and animated. Our terminal version can only approximate:
- **Tradeoff**: We cannot draw proportional rectangles in a terminal (line height = 1 cell; non-integer ratios alias badly). We will show stacked equal-size boxes labeled with their actual dimensions. The visual hierarchy is preserved (which children are big vs small is communicated via labels not pixel area).
- **Mitigation**: Optionally, render a horizontal proportional bar at the top showing the relative main-axis sizes of the children (one row of `█` cells colored per child) so the user still gets a quick proportion read. Defer to Phase D implementation review.

### 7.2 `getProperties` RPC chattiness

For one Enter press we issue up to 1 + N additional `getProperties` calls (one for the widget, one for each render-object property — typically 0 or 1). This is the same pattern DevTools uses. Two safeguards:
- Re-use the existing `fdemon-inspector-N` object group for the lifetime of a details view (matches DevTools behavior).
- Cache the fetched `properties` / `render_properties` per `details_node_id`. If the user closes + reopens Details on the same node, no re-fetch.

### 7.3 Chain-collapse heuristic deviation

DevTools' `_alwaysVisible` has a subtle "has siblings" branch that means a non-local-project node with siblings is ALWAYS visible, even if it's a wrapper widget. We will replicate this exactly to avoid drifting from DevTools' look-and-feel; users moving between fdemon and DevTools shouldn't see two different trees for the same app.

### 7.4 Esc semantics change

Today `Esc` exits DevTools. New behavior: `Esc` in Details closes Details, second `Esc` exits. This is a minor UX change but is consistent with how settings + dialogs already work. Documented in `KEYBINDINGS.md`.

### 7.5 RequestTracker integration

`FetchInspectorProperties` will follow the same pattern as `FetchLayoutData` — spawn a task that calls `vm_service.get_properties(...)`, then sends a result `Message` back to the engine. No new RequestTracker entries beyond the standard inspector-extension pattern (the RequestTracker handles the JSON-RPC request/response matching transparently; new extension calls don't need RequestTracker changes).

### 7.6 Frozen selection while Details open (resolved)

Decision: **frozen** (§3.1 #1). `Up` / `Down` are no-ops while `details_open == true`; selection only moves after `Esc` returns to tree mode. This avoids per-keypress `getProperties` re-fetches and keeps the keymap unambiguous between "navigate tree" and "use details panel."

## 8. Out-of-Scope (Future Work)

- "Hide implementation widgets" settings toggle (DevTools' isSummaryTree on/off switch).
- On-device selection via `setSelectionById` (clicking in fdemon → highlight in running Flutter app).
- Inline expandable property values (e.g., `Color` → ARGB picker).
- Mouse drag-to-resize the tree/details split.

## 9. Documentation Updates

| Doc | Owner | Trigger |
|---|---|---|
| `docs/ARCHITECTURE.md` — DevTools Subsystem section | `doc_maintainer` agent | Phase B (adds DetailsTab state model) + Phase C (adds `getProperties` extension to the VM Service list) |
| `docs/KEYBINDINGS.md` — Inspector Panel section | implementor (unmanaged doc) | Phase B |
| `docs/REVIEW_FOCUS.md` — Approved exceptions | n/a — no new TEA exceptions | — |

## 10. Phased Checklist

- [ ] **Phase 1 — Tree rendering + hide-impl toggle + details scaffold** (`phase-1/`)
  - 1A: Guideline tree, chain collapse, per-type icon, `Shift+H` toggle for hide-implementation, settings persistence.
  - 1B: `Enter`-to-details flow, tabbed details panel with Widget properties tab populated; Render Object and Flex Explorer stubs show "Coming soon"; frozen selection while details open; tiered Esc semantics.
- [ ] **Phase 2 — Render object & Flex explorer tabs** (`phase-2/`)
  - 2A: New `getProperties` VM Service extension call (`fdemon-daemon/src/vm_service/extensions/inspector.rs`), two-stage widget/render-object fetch, populated Render object tab.
  - 2B: Extended `LayoutInfo.children` parsing from `getLayoutExplorerNode`, ASCII Flex Explorer renderer.
- [ ] **Phase 3 — Conditional tab visibility + polish** (`phase-3/`)
  - Per-widget-type tab list: Container → 1 tab; non-flex render widget → 2 tabs; Row/Column/Flex (or their direct children) → 3 tabs.
  - Tab-cycling skips hidden tabs.
  - Snapshot tests covering each widget-type case.

After this plan is approved, the task index for **Phase 1** will be created at `workflow/plans/features/devtools-inspector-parity/phase-1/TASKS.md` with per-file tasks and the required File Overlap Analysis. Phase 2 and Phase 3 task indexes will be created after their preceding phase completes (so we can adjust based on review feedback / new findings).
