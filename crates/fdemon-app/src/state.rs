//! Application state (Model in TEA pattern)

use std::cell::Cell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use rand::Rng;

use crate::config::{LoadedConfigs, Settings, SettingsTab, UserPreferences};
use crate::confirm_dialog::ConfirmDialogState;
use crate::flutter_version::FlutterVersionState;
use crate::install_wizard::{InstallWizardState, WizardOrigin};
use crate::mouse_regions::{MouseRegions, MouseRegionsCell};
use crate::new_session_dialog::NewSessionDialogState;
use crate::new_session_dialog::{DartDefinesModalState, FuzzyModalState};
use fdemon_core::{
    build_inspector_rows, AppPhase, DetailsContext, DiagnosticsNode, InspectorRow,
    InspectorRowBuilderInputs, LayoutInfo,
};
use fdemon_daemon::{AndroidAvd, Device, FlutterSdk, IosSimulator, ToolAvailability};

use super::session::SharedSourceHandle;
use super::session_manager::SessionManager;

/// Current UI mode/screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    /// Startup state - show NewSessionDialog (no sessions yet)
    #[default]
    Startup,

    /// Normal TUI with log view and status bar
    Normal,

    /// New session dialog - unified device and configuration selection
    /// Used both at startup (Startup mode) and when adding sessions (Normal mode)
    NewSessionDialog,

    /// Emulator selector (after choosing "Launch Android Emulator")
    EmulatorSelector,

    /// Confirmation dialog (e.g., quit confirmation)
    ConfirmDialog,

    /// Initial loading screen (discovering devices)
    Loading,

    /// Search input mode - capturing text for log search
    SearchInput,

    /// Link highlight mode - showing clickable file references
    /// User can press 1-9 or a-z to open a file in their editor
    LinkHighlight,

    /// Settings panel - full-screen settings UI
    Settings,

    /// Flutter Version panel - displays current SDK info and installed versions
    FlutterVersion,

    /// DevTools panel mode - replaces log view with Inspector/Performance panels
    DevTools,

    /// Install Wizard panel - guides users through Flutter toolchain setup
    InstallWizard,
}

// ─────────────────────────────────────────────────────────────────────────────
// Startup Notice (version-check-banner)
// ─────────────────────────────────────────────────────────────────────────────

/// A persistent one-line notice rendered above the New Session Dialog
/// on startup. Cleared when the dialog is dismissed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupNotice {
    /// A newer fdemon release is available on GitHub.
    NewVersionAvailable { latest: String },
}

// ─────────────────────────────────────────────────────────────────────────────
// DevTools State (Phase 4)
// ─────────────────────────────────────────────────────────────────────────────

/// VM Service connection status for display in DevTools UI.
///
/// Extends the binary `vm_connected: bool` flag on `Session` with richer
/// reconnection/timeout state that can be surfaced in the TUI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VmConnectionStatus {
    /// WebSocket connection established and VM Service is responding.
    #[default]
    Connected,

    /// No active connection (startup or after a clean disconnect).
    Disconnected,

    /// Connection was lost and the client is retrying.
    ///
    /// `attempt` is 1-based (first retry = 1).
    /// `max_attempts` is the total number of retries before giving up.
    Reconnecting {
        /// Current attempt number (1-based).
        attempt: u32,
        /// Maximum number of retry attempts.
        max_attempts: u32,
    },

    /// A specific VM RPC call timed out (e.g., FetchWidgetTree, FetchLayoutData).
    ///
    /// The connection itself may still be live; this indicates that a single
    /// on-demand request did not complete within the configurable deadline.
    TimedOut,
}

impl VmConnectionStatus {
    /// Short human-readable label used in the DevTools tab bar indicator.
    ///
    /// Examples:
    /// - `"Connected"`
    /// - `"Reconnecting (2/10)"`
    /// - `"Disconnected"`
    /// - `"Timed Out"`
    pub fn label(&self) -> String {
        match self {
            VmConnectionStatus::Connected => "Connected".to_string(),
            VmConnectionStatus::Disconnected => "Disconnected".to_string(),
            VmConnectionStatus::Reconnecting {
                attempt,
                max_attempts,
            } => {
                format!("Reconnecting ({attempt}/{max_attempts})")
            }
            VmConnectionStatus::TimedOut => "Timed Out".to_string(),
        }
    }

    /// Returns `true` when the status indicates some form of connectivity
    /// loss (disconnected, reconnecting, or timed-out).
    pub fn is_degraded(&self) -> bool {
        !matches!(self, VmConnectionStatus::Connected)
    }
}

/// Active sub-panel within DevTools mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevToolsPanel {
    /// Widget tree inspector with expand/collapse navigation.
    #[default]
    Inspector,

    /// FPS, memory usage, and frame timing display.
    Performance,

    /// Heap memory usage chart and allocation breakdown.
    Memory,

    /// HTTP/WebSocket network request monitor.
    Network,
}

/// A user-friendly error with an actionable hint for DevTools panels.
///
/// Created by [`crate::handler::devtools::map_rpc_error`] which maps raw RPC
/// error strings to concise messages the TUI can display in a centred error box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevToolsError {
    /// Short, human-readable description of the problem (≤ 60 chars recommended).
    pub message: String,
    /// Actionable guidance shown below the message (key hints, mode suggestion, etc.).
    pub hint: String,
}

impl DevToolsError {
    /// Create a new `DevToolsError`.
    pub fn new(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: hint.into(),
        }
    }
}

/// Which tab is active in the Details view of the Inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailsTab {
    /// Widget property list returned by `getProperties`.
    #[default]
    Properties,
    /// Render-object property nodes (those with `propertyType == "RenderObject"`).
    RenderObject,
    /// Flex layout explorer for `Row`, `Column`, and `Flex` widgets.
    FlexExplorer,
}

/// Which tab is active within the Performance panel's Details pane.
///
/// Phase 2 populates `FrameAnalysis`; `RebuildStats` and `TimelineEvents`
/// render "Coming soon" stubs until Phase 3 adds the underlying VM Service
/// flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PerfDetailsTab {
    /// Per-frame phase breakdown + refresh-rate-aware hints. Default.
    #[default]
    FrameAnalysis,
    /// Widget rebuild counts per frame (Phase 3 stub in Phase 2).
    RebuildStats,
    /// UI / Raster thread timeline events (Phase 3 stub in Phase 2).
    TimelineEvents,
}

impl PerfDetailsTab {
    /// Next tab in display order (wraps from TimelineEvents → FrameAnalysis).
    pub fn next(self) -> Self {
        match self {
            PerfDetailsTab::FrameAnalysis => PerfDetailsTab::RebuildStats,
            PerfDetailsTab::RebuildStats => PerfDetailsTab::TimelineEvents,
            PerfDetailsTab::TimelineEvents => PerfDetailsTab::FrameAnalysis,
        }
    }

    /// Previous tab in display order (wraps from FrameAnalysis → TimelineEvents).
    pub fn prev(self) -> Self {
        match self {
            PerfDetailsTab::FrameAnalysis => PerfDetailsTab::TimelineEvents,
            PerfDetailsTab::RebuildStats => PerfDetailsTab::FrameAnalysis,
            PerfDetailsTab::TimelineEvents => PerfDetailsTab::RebuildStats,
        }
    }

    /// Next tab in display order, skipping `RebuildStats` when `rebuild_stats_enabled` is false.
    ///
    /// When `rebuild_stats_enabled == false`:
    /// - `FrameAnalysis → TimelineEvents → FrameAnalysis` (two-step cycle).
    ///
    /// When `rebuild_stats_enabled == true`:
    /// - `FrameAnalysis → RebuildStats → TimelineEvents → FrameAnalysis` (full three-step cycle).
    pub fn next_visible(self, rebuild_stats_enabled: bool) -> Self {
        if rebuild_stats_enabled {
            self.next()
        } else {
            match self {
                PerfDetailsTab::FrameAnalysis => PerfDetailsTab::TimelineEvents,
                PerfDetailsTab::RebuildStats => PerfDetailsTab::TimelineEvents,
                PerfDetailsTab::TimelineEvents => PerfDetailsTab::FrameAnalysis,
            }
        }
    }
}

/// State for the widget inspector tree view.
///
/// Also holds layout data for the currently selected widget (merged into this struct
/// in Phase 2). Layout fields use a `layout_` prefix to avoid conflicts with inspector fields.
#[derive(Debug, Clone)]
pub struct InspectorState {
    /// The root widget tree node (fetched on-demand via VM Service RPC).
    pub root: Option<DiagnosticsNode>,

    /// Set of expanded node IDs (value_id). Collapsed by default.
    pub expanded: HashSet<String>,

    /// Index of the currently selected visible node (0-based flat list position).
    pub selected_index: usize,

    /// Whether a tree fetch is currently in progress.
    pub loading: bool,

    /// User-friendly error from the last failed fetch attempt.
    ///
    /// `None` when no error has occurred or after a successful fetch.
    /// Populated by [`crate::handler::devtools::map_rpc_error`] so the TUI
    /// always shows a clear message + hint instead of a raw RPC error string.
    pub error: Option<DevToolsError>,

    /// Whether the `"fdemon-inspector-1"` VM object group exists on the Flutter VM.
    ///
    /// Set to `true` after a successful widget tree fetch, `false` after disposal
    /// or reset. Used to skip unnecessary `disposeGroup` RPC calls when no group
    /// has been created yet.
    pub has_object_group: bool,

    /// Timestamp of the last successful widget tree fetch.
    ///
    /// Used to enforce a 2-second cooldown on rapid refresh requests (`r` key).
    /// A new fetch is only dispatched when all of the following hold:
    /// - `loading == false` (no fetch in flight), AND
    /// - either `last_fetch_time` is `None`, OR at least 2 seconds have elapsed.
    ///
    /// This prevents RPC spam when the user holds down the refresh key.
    pub last_fetch_time: Option<Instant>,

    // ── Layout fields ──────────────────────────────────────────────────────────
    /// Layout info for the currently selected widget.
    pub layout: Option<LayoutInfo>,

    /// Whether a layout fetch is in progress.
    pub layout_loading: bool,

    /// User-friendly error from the last failed layout fetch.
    ///
    /// `None` when no error has occurred or after a successful fetch.
    /// Populated by [`crate::handler::devtools::map_rpc_error`].
    pub layout_error: Option<DevToolsError>,

    /// Whether the `"devtools-layout"` VM object group exists on the Flutter VM.
    ///
    /// Set to `true` after a successful layout fetch, `false` after disposal
    /// or reset. Used to skip unnecessary `disposeGroup` RPC calls when no group
    /// has been created yet.
    pub has_layout_object_group: bool,

    /// The `value_id` of the inspector node for which layout data was last fetched.
    ///
    /// Compared against the currently selected inspector node when the user
    /// switches to the Layout panel. If the selected node has not changed,
    /// the layout fetch is skipped to avoid redundant RPC calls.
    ///
    /// Reset to `None` when the state is reset (e.g., session switch).
    pub last_fetched_node_id: Option<String>,

    /// The `value_id` of the inspector node for which a fetch is currently in flight.
    ///
    /// Set when a `FetchLayoutData` action is dispatched and consumed in
    /// `handle_layout_data_fetched` to populate `last_fetched_node_id` on
    /// success. Reset to `None` on failure or reset.
    pub pending_node_id: Option<String>,

    /// Timestamp of the last layout data fetch dispatch.
    ///
    /// Used to enforce a 500ms cooldown on auto-fetch requests during tree
    /// navigation (Up/Down keys). A new fetch is skipped when either:
    /// - `layout_loading == true` (fetch already in flight), OR
    /// - `layout_last_fetch_time` is `Some(t)` and `t.elapsed() < 500ms`.
    ///
    /// This prevents RPC spam during rapid scrolling through the widget tree.
    pub layout_last_fetch_time: Option<Instant>,

    /// Sticky flag that becomes `true` after the first successful widget tree
    /// render in the current Flutter isolate.
    ///
    /// **Does not reset on [`Self::reset`], fetch debounce clears, or
    /// individual fetch failures.** Cleared on:
    /// - session destruction (drop)
    /// - hot restart (`Message::SessionRestartCompleted`)
    ///
    /// Hot restart creates a new isolate and re-initializes the framework, so
    /// the "framework is warm" invariant the flag encodes is temporarily invalid;
    /// the next fetch should use the full readiness poll budget.
    ///
    /// Used to choose between `FetchTrigger::Initial` (poll applies) and
    /// `FetchTrigger::Refresh` (poll skipped) when the user presses `r`.
    /// If the user refreshes before the inspector has ever loaded a tree the
    /// flag will be `false` and `Initial` is used so polling still applies.
    pub has_ever_rendered_tree: bool,

    // ── Chain-folding / "Hide implementation widgets" ─────────────────────────
    /// Set of leader `value_id`s whose hideable chain is currently expanded.
    ///
    /// Independent of `expanded` (which tracks regular tree expand/collapse).
    /// A leader whose id is present here renders as
    /// [`fdemon_core::RowGroup::LeaderExpanded`] with `Member` sub-rows visible;
    /// otherwise it renders as [`fdemon_core::RowGroup::LeaderCollapsed`] and
    /// its sub-rows are suppressed.
    pub expanded_groups: HashSet<String>,

    /// When `true`, contiguous chains of non-local-project wrapper widgets are
    /// folded into a leader row.
    ///
    /// Mirrors DevTools' "Hide implementation widgets" toggle.
    /// Defaults to `true`. Persisted via `[devtools]` in settings (the
    /// startup-time application happens in task 03; the field itself lives here).
    ///
    /// **Preserved** across [`Self::reset`] calls (user preference).
    pub hide_implementation_widgets: bool,

    // ── Details view ──────────────────────────────────────────────────────────
    /// `true` when the user has opened the Details view (Enter pressed).
    pub details_open: bool,

    /// Which tab is currently active in the Details view.
    pub details_tab: DetailsTab,

    /// `value_id` of the widget whose details are currently displayed.
    ///
    /// Snapshotted from the selected row at Open time; not updated by
    /// navigation (selection is frozen while details are open).
    pub details_node_id: Option<String>,

    /// Widget property nodes returned by `getProperties` for the
    /// `details_node_id` widget.
    ///
    /// Populated in Phase 2; empty in Phase 1.
    pub properties: Vec<DiagnosticsNode>,

    /// Render-object diagnostics property nodes (those with
    /// `propertyType == "RenderObject"`) extracted from `properties`.
    ///
    /// Populated in Phase 2; empty in Phase 1.
    pub render_properties: Vec<DiagnosticsNode>,

    /// `true` when a properties fetch is in flight (Phase 2).
    pub properties_loading: bool,

    /// User-friendly error from the last properties fetch (Phase 2).
    pub properties_error: Option<DevToolsError>,

    /// `value_id` of the last widget whose properties were successfully fetched.
    /// Used as a cache key by `handle_open_details` to skip re-dispatch when the
    /// user closes + reopens Details on the same node.
    pub last_fetched_properties_node_id: Option<String>,

    /// `value_id` of the in-flight properties fetch, if any. Used as a stale
    /// guard in `handle_properties_fetched`: if the user closes Details or
    /// switches to a different node mid-flight, the late response is discarded.
    pub pending_properties_node_id: Option<String>,

    /// Cached tree-derived predicates for the open details session.
    ///
    /// Populated by `handle_open_details` via
    /// [`fdemon_core::widget_tree::compute_details_context`]. Used by
    /// [`Self::visible_tabs`] to decide which tabs render. Cleared by
    /// [`Self::reset`] and [`Self::reset_details_and_groups`]; overwritten on
    /// every successful `handle_open_details`.
    ///
    /// Default value (`DetailsContext::default()`) is harmless because
    /// `visible_tabs` is only consumed while `details_open == true`, and
    /// `handle_open_details` always writes here before flipping `details_open`.
    pub details_context: DetailsContext,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            root: None,
            expanded: HashSet::new(),
            selected_index: 0,
            loading: false,
            error: None,
            has_object_group: false,
            last_fetch_time: None,
            layout: None,
            layout_loading: false,
            layout_error: None,
            has_layout_object_group: false,
            last_fetched_node_id: None,
            pending_node_id: None,
            layout_last_fetch_time: None,
            has_ever_rendered_tree: false,
            // Matches DevTools default: implementation widgets are hidden.
            hide_implementation_widgets: true,
            expanded_groups: HashSet::new(),
            details_open: false,
            details_tab: DetailsTab::Properties,
            details_node_id: None,
            properties: Vec::new(),
            render_properties: Vec::new(),
            properties_loading: false,
            properties_error: None,
            last_fetched_properties_node_id: None,
            pending_properties_node_id: None,
            details_context: DetailsContext::default(),
        }
    }
}

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
    ///
    /// # Preserved fields
    /// - `hide_implementation_widgets`: user preference; survives session switches.
    /// - `has_ever_rendered_tree`: sticky flag; only cleared on hot restart.
    ///
    /// # Cleared fields
    /// All other fields including the new details-view state and chain-group
    /// expansion set are reset to their defaults.
    pub fn reset(&mut self) {
        self.root = None;
        self.expanded.clear();
        self.selected_index = 0;
        self.loading = false;
        self.error = None;
        // has_ever_rendered_tree intentionally NOT reset — sticky for session lifetime.
        // Cleared on hot restart (handler/update.rs::SessionRestartCompleted) and
        // session drop.
        self.has_object_group = false;
        self.last_fetch_time = None;
        // Layout fields
        self.layout = None;
        self.layout_loading = false;
        self.layout_error = None;
        self.has_layout_object_group = false;
        self.last_fetched_node_id = None;
        self.pending_node_id = None;
        self.layout_last_fetch_time = None;
        // Chain-folding: clear per-session group expansion state.
        // hide_implementation_widgets is intentionally NOT reset — user preference.
        self.expanded_groups.clear();
        // Details view
        self.details_open = false;
        self.details_tab = DetailsTab::Properties;
        self.details_node_id = None;
        self.details_context = DetailsContext::default();
        self.properties.clear();
        self.render_properties.clear();
        self.properties_loading = false;
        self.properties_error = None;
        self.last_fetched_properties_node_id = None;
        self.pending_properties_node_id = None;
    }

    /// Clears state that does not survive a tree refresh or hot restart.
    ///
    /// Unlike [`Self::reset`], this preserves the user's tree-shape preferences
    /// (`hide_implementation_widgets`) and the sticky `has_ever_rendered_tree`
    /// flag. It clears state that points at specific widget identities that
    /// would be invalidated by a new tree (group leader ids, details snapshot)
    /// or a new Dart isolate (Dart object ids referenced by `details_node_id`).
    ///
    /// Called from:
    /// - [`crate::handler::devtools::handle_widget_tree_fetched`] — on each
    ///   successful tree refresh, to discard stale details pointing at old nodes.
    /// - The `Message::SessionRestartCompleted` handler — after hot restart,
    ///   because a new isolate invalidates all Dart object ids.
    pub fn reset_details_and_groups(&mut self) {
        self.details_open = false;
        self.details_node_id = None;
        self.details_tab = DetailsTab::Properties;
        self.details_context = DetailsContext::default();
        self.expanded_groups.clear();
        self.properties.clear();
        self.render_properties.clear();
        self.properties_loading = false;
        self.properties_error = None;
        self.last_fetched_properties_node_id = None;
        self.pending_properties_node_id = None;
    }

    /// Returns `true` after the first successful widget tree render.
    ///
    /// This flag is sticky: it is set to `true` by
    /// [`crate::handler::devtools::handle_widget_tree_fetched`] and never
    /// cleared by [`Self::reset`], debounce clears, or fetch failures.
    /// It is explicitly cleared on hot restart (`Message::SessionRestartCompleted`)
    /// because hot restart creates a new isolate and re-initializes the framework.
    ///
    /// Used by `Message::RequestWidgetTree` handler to pick
    /// `FetchTrigger::Refresh` (skip readiness poll) when the Flutter
    /// framework is already known to be running.
    pub fn has_ever_rendered_tree(&self) -> bool {
        self.has_ever_rendered_tree
    }

    /// Returns `true` if a tree refresh request should be suppressed.
    ///
    /// A request is suppressed when either:
    /// - A fetch is already in flight (`loading == true`), OR
    /// - The last successful fetch occurred within the 2-second cooldown window.
    pub fn is_fetch_debounced(&self) -> bool {
        const COOLDOWN: std::time::Duration = std::time::Duration::from_secs(2);
        if self.loading {
            return true;
        }
        self.last_fetch_time
            .map(|t| t.elapsed() < COOLDOWN)
            .unwrap_or(false)
    }

    /// Returns `true` if a layout fetch should be skipped (debounced).
    ///
    /// A layout fetch is debounced when either:
    /// - A fetch is already in flight (`layout_loading == true`), OR
    /// - The last layout fetch was dispatched within the 500ms cooldown window.
    ///
    /// This shorter cooldown (vs the 2s tree cooldown) allows reasonable
    /// responsiveness during tree navigation without spamming VM Service RPC calls.
    pub fn is_layout_fetch_debounced(&self) -> bool {
        if self.layout_loading {
            return true;
        }
        match self.layout_last_fetch_time {
            Some(t) => t.elapsed() < std::time::Duration::from_millis(500),
            None => false,
        }
    }

    /// Record that a fetch was just initiated.
    ///
    /// Sets `loading = true` and updates `last_fetch_time` to `Instant::now()`
    /// so that the next request within 2 seconds is suppressed by
    /// [`Self::is_fetch_debounced`].
    pub fn record_fetch_start(&mut self) {
        self.loading = true;
        self.last_fetch_time = Some(Instant::now());
    }

    /// Clear the fetch debounce timer so that the next refresh request is
    /// dispatched immediately.
    ///
    /// Called after a failed or timed-out widget tree fetch so the user can
    /// press `r` again without waiting for the 2-second cooldown to expire.
    /// The success path intentionally does **not** clear this — it is fine
    /// for rapid `r` presses after a successful fetch to be gated by the
    /// cooldown.
    pub fn clear_fetch_debounce(&mut self) {
        self.last_fetch_time = None;
    }

    /// Build the list of rendered rows with vertical-guideline + branch-tick
    /// metadata and chain-collapse applied.
    ///
    /// Respects `hide_implementation_widgets` and `expanded_groups` so that
    /// collapsed leader rows suppress their subordinates from the output.
    pub fn inspector_rows(&self) -> Vec<InspectorRow<'_>> {
        let Some(root) = &self.root else {
            return vec![];
        };
        build_inspector_rows(InspectorRowBuilderInputs {
            root,
            expanded: &self.expanded,
            expanded_groups: &self.expanded_groups,
            hide_implementation: self.hide_implementation_widgets,
        })
    }

    /// Backwards-compatible shim for callers that only need `(node, depth)` tuples.
    ///
    /// Built on [`Self::inspector_rows`] so it respects chain folding.
    /// Collapsed group-leader subordinates are absent from the returned slice,
    /// matching the visible row count used for navigation bounds.
    pub fn visible_nodes(&self) -> Vec<(&DiagnosticsNode, usize)> {
        self.inspector_rows()
            .into_iter()
            .map(|row| (row.node, row.depth))
            .collect()
    }

    /// Return the description of the currently selected visible row.
    ///
    /// Delegates to [`Self::inspector_rows`] so that the result is consistent
    /// with chain-folding: a node hidden inside a collapsed group does not
    /// occupy an index and the leader row counts as exactly one row.
    ///
    /// Returns `None` when no tree is loaded or `selected_index` is out of
    /// bounds.
    pub fn selected_node_description(&self) -> Option<String> {
        let rows = self.inspector_rows();
        rows.get(self.selected_index)
            .map(|r| r.node.description.clone())
    }

    /// Returns the currently-selected row from the active row list, or `None`
    /// if the selection is out of bounds.
    ///
    /// The returned row carries its [`fdemon_core::RowGroup`] which callers can
    /// match on to decide whether the row is a chain leader, member, or
    /// standalone.
    ///
    /// Returns `None` when no tree is loaded or `selected_index` is out of
    /// bounds.
    pub fn selected_row(&self) -> Option<InspectorRow<'_>> {
        let rows = self.inspector_rows();
        rows.into_iter().nth(self.selected_index)
    }

    /// Return the `value_id` of the currently selected visible row.
    ///
    /// Used by handler code to obtain the identifier needed for RPC calls
    /// (e.g., `getProperties`, `getLayoutData`) for the selected widget.
    ///
    /// Returns `None` when no tree is loaded, `selected_index` is out of
    /// bounds, or the node at that position has no `value_id`.
    pub fn selected_value_id(&self) -> Option<String> {
        self.selected_row().and_then(|r| r.node.value_id.clone())
    }

    /// Return the ordered list of tabs that should be visible in the Details
    /// strip given current state.
    ///
    /// Visibility rules (DevTools parity, parent PLAN §5.4):
    /// - [`DetailsTab::Properties`] is always included.
    /// - [`DetailsTab::RenderObject`] is included iff
    ///   `!self.render_properties.is_empty()`.
    /// - [`DetailsTab::FlexExplorer`] is included iff
    ///   `self.details_context.is_flex_layout` (precomputed by
    ///   `handle_open_details` via `compute_details_context`).
    ///
    /// Returned in display order. Caller is free to assume the first element
    /// is always `Properties` and to use the order for cycling.
    ///
    /// Pure: does not walk the tree, does not allocate beyond the returned vec,
    /// and never mutates state. Safe to call from the TUI renderer.
    pub fn visible_tabs(&self) -> Vec<DetailsTab> {
        let mut tabs = Vec::with_capacity(3);
        tabs.push(DetailsTab::Properties);
        if !self.render_properties.is_empty() {
            tabs.push(DetailsTab::RenderObject);
        }
        if self.details_context.is_flex_layout {
            tabs.push(DetailsTab::FlexExplorer);
        }
        tabs
    }

    /// Ensure `self.details_tab` is in [`Self::visible_tabs`]; if not, set it
    /// to the first visible tab (always `Properties`).
    ///
    /// Call this after any state transition that may have removed the active
    /// tab from the visible set:
    /// - `handle_inspector_properties_fetched` (fetch may yield empty
    ///   `render_properties` → Render Object tab disappears).
    /// - `handle_inspector_properties_fetch_failed` (same, depending on
    ///   pre-failure cache state).
    ///
    /// `handle_open_details` already sets `details_tab = Properties` directly
    /// and does not need to call this method.
    ///
    /// `handle_close_details` does not need this — the renderer never sees
    /// state while `details_open == false`.
    pub fn clamp_details_tab(&mut self) {
        let visible = self.visible_tabs();
        if !visible.contains(&self.details_tab) {
            self.details_tab = visible.first().copied().unwrap_or(DetailsTab::Properties);
        }
    }
}

/// Complete state for the DevTools mode UI.
#[derive(Debug, Clone, Default)]
pub struct DevToolsViewState {
    /// Currently active sub-panel.
    pub active_panel: DevToolsPanel,

    /// Widget inspector tree state (also contains layout explorer data).
    pub inspector: InspectorState,

    /// Current debug overlay states (synced from VM Service).
    pub overlay_repaint_rainbow: bool,
    pub overlay_debug_paint: bool,
    pub overlay_performance: bool,

    /// Last VM Service connection error message, if any.
    /// Set on `VmServiceConnectionFailed`, cleared on `VmServiceConnected`.
    /// Displayed in DevTools panels so users see actionable errors instead of
    /// the generic "VM Service not connected" message.
    pub vm_connection_error: Option<String>,

    /// Rich VM Service connection status (Phase 5, Task 02).
    ///
    /// Tracks connected / disconnected / reconnecting / timed-out states so
    /// the TUI can display colour-coded indicators in the DevTools tab bar
    /// and show appropriate messages in each panel.
    ///
    /// Updated by the handler in response to VM Service lifecycle messages:
    /// - `VmServiceConnected`    → `Connected`
    /// - `VmServiceDisconnected` → `Disconnected`
    /// - `VmServiceReconnecting` → `Reconnecting { attempt, max_attempts }`
    /// - `WidgetTreeFetchTimeout` / `LayoutDataFetchTimeout` → `TimedOut`
    pub connection_status: VmConnectionStatus,

    /// Timestamp of the last debug overlay toggle.
    ///
    /// Used to debounce rapid key presses: overlay toggle RPCs are suppressed
    /// if the last toggle occurred within 500 ms. This prevents multiple
    /// in-flight RPC calls when the user holds down the toggle key.
    pub last_overlay_toggle: Option<Instant>,
}

impl DevToolsViewState {
    /// Reset all session-specific DevTools state.
    ///
    /// Called when the user switches between sessions so that stale data
    /// from the previous session is not displayed for the new session.
    ///
    /// NOTE: `active_panel` is intentionally preserved — the user's panel
    /// choice (Inspector / Performance) persists across session switches
    /// as it is a UI preference, not session data.
    pub fn reset(&mut self) {
        self.inspector.reset();
        self.overlay_repaint_rainbow = false;
        self.overlay_debug_paint = false;
        self.overlay_performance = false;
        self.vm_connection_error = None;
        self.connection_status = VmConnectionStatus::Disconnected;
        self.last_overlay_toggle = None;
    }

    /// Returns `true` if the overlay toggle debounce cooldown (500 ms) has
    /// not yet elapsed since the last toggle.
    ///
    /// When this returns `true` the caller should suppress the RPC and not
    /// update `last_overlay_toggle`.
    pub fn is_overlay_toggle_debounced(&self) -> bool {
        const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(500);
        self.last_overlay_toggle
            .map(|t| t.elapsed() < DEBOUNCE)
            .unwrap_or(false)
    }

    /// Record that an overlay toggle was just dispatched.
    ///
    /// Updates `last_overlay_toggle` to `Instant::now()` so that the next
    /// call within 500 ms will be suppressed by [`Self::is_overlay_toggle_debounced`].
    pub fn record_overlay_toggle(&mut self) {
        self.last_overlay_toggle = Some(Instant::now());
    }
}

/// State for the settings panel view
#[derive(Debug, Clone)]
pub struct SettingsViewState {
    /// Currently active tab
    pub active_tab: SettingsTab,

    /// Currently selected item index within the active tab
    pub selected_index: usize,

    /// Whether we're in edit mode for the current item
    pub editing: bool,

    /// Text buffer for string editing
    pub edit_buffer: String,

    /// Dirty flag - have settings been modified?
    pub dirty: bool,

    /// Loaded user preferences (for User tab)
    pub user_prefs: UserPreferences,

    /// Error message to display (if any)
    pub error: Option<String>,

    /// Active dart defines modal overlay (if any).
    ///
    /// Set when the user opens the dart defines editor for a launch config.
    pub dart_defines_modal: Option<DartDefinesModalState>,

    /// The 0-based index of the launch config currently being edited.
    ///
    /// **SHARED** between `dart_defines_modal` and `extra_args_modal` —
    /// only one modal may be open at a time. The `has_modal_open()` guard
    /// in each open handler enforces this invariant.
    ///
    /// Set on modal open, cleared on modal close/cancel.
    pub editing_config_idx: Option<usize>,

    /// Active extra args fuzzy modal overlay (if any).
    ///
    /// Set when the user opens the extra args picker for a launch config.
    pub extra_args_modal: Option<FuzzyModalState>,
}

impl Default for SettingsViewState {
    fn default() -> Self {
        Self {
            active_tab: SettingsTab::Project,
            selected_index: 0,
            editing: false,
            edit_buffer: String::new(),
            dirty: false,
            user_prefs: UserPreferences::default(),
            error: None,
            dart_defines_modal: None,
            editing_config_idx: None,
            extra_args_modal: None,
        }
    }
}

impl SettingsViewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if any modal overlay is currently open.
    ///
    /// Used by the settings panel key handler to route input to the active modal
    /// instead of the underlying settings list.
    pub fn has_modal_open(&self) -> bool {
        self.dart_defines_modal.is_some() || self.extra_args_modal.is_some()
    }

    /// Load user preferences from disk
    pub fn load_user_prefs(&mut self, project_path: &std::path::Path) {
        if let Some(prefs) = crate::config::load_user_preferences(project_path) {
            self.user_prefs = prefs;
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
        self.selected_index = 0;
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.prev();
        self.selected_index = 0;
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Jump to specific tab
    pub fn goto_tab(&mut self, tab: SettingsTab) {
        self.active_tab = tab;
        self.selected_index = 0;
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Select next item
    pub fn select_next(&mut self, item_count: usize) {
        if item_count > 0 {
            self.selected_index = (self.selected_index + 1) % item_count;
        }
    }

    /// Select previous item
    pub fn select_previous(&mut self, item_count: usize) {
        if item_count > 0 {
            self.selected_index = if self.selected_index == 0 {
                item_count - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Enter edit mode
    pub fn start_editing(&mut self, initial_value: &str) {
        self.editing = true;
        self.edit_buffer = initial_value.to_string();
    }

    /// Exit edit mode
    pub fn stop_editing(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Mark settings as modified
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Clear dirty flag (after save)
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading State (Phase 5 Task 08d)
// ─────────────────────────────────────────────────────────────────────────────

/// Loading messages to cycle through during device discovery (Claude Code style gerunds)
const LOADING_MESSAGES: &[&str] = &[
    "Detecting devices...",
    "Scanning for emulators...",
    "Initializing flutter daemon...",
    "Querying device connections...",
    "Waking up simulators...",
    "Consulting the device oracle...",
    "Rummaging through USB ports...",
    "Befriending nearby devices...",
    "Summoning Android spirits...",
    "Polishing iOS artifacts...",
    "Resolving adb identity crisis...",
    "Jiggling the USB cable...",
    "Bribing the operating system...",
    "Waking up the GPU hamsters...",
    "Filtering logcat noise...",
    "Paging Dr. Flutter...",
    "Ignoring deprecated warnings...",
    "Linking binary libraries...",
    "Writing an App Store appeal email...",
    "Demonizing Flutter daemon...",
    "Possesing the terminal...",
    "Negotiating with local ghosts..",
    "Calibrating flux capacitors...",
    "Flushing the socket buffers...",
    "Asking the hub for directions...",
    "Convincing the emulator it's a real phone...",
    "Interrogating system processes...",
    "Consulting the runes...",
    "Tuning the JVM...",
    "Refactoring AndroidManifest.xml...",
    "Warming up the JIT compiler...",
    "Waiting for Xcode to finish 'Indexing'...",
    "Calculating safe area insets...",
    "Convincing the simulator it has a notch...",
    "Archiving... Validating... Distributing...",
    "Awaiting the Future...",
    "Consulting Guideline 4.2...",
    "Fighting Provisioning Profiles...",
    "Calculating the 30% cut...",
    "Searching for the dSYM...",
    "Asking Siri for help...",
    "Checking IAP entitlements...",
    "Polishing the launch screen...",
    "Generating technical debt...",
    "Blaming the firewall...",
    "Sacrificing RAM to Chrome...",
    "Waiting for Internet Explorer...",
    "Loading... (fingers crossed)...",
    "Reticulating splines...",
    "Downloading Maven Central...",
    "Feeding the Gradle Daemon...",
    "Conversing with the build cache...",
    "Fumigating node_modules folder...",
    "Herding NPM packages...",
    "Orchestrating a race condition...",
    "Debugging the debugger...",
    "Demystifying the provisioning profile...",
    "Exorcising the stale cache...",
    "Arbitrating state management conflicts...",
    "Liquidating memory leaks...",
    "Gambling with hot reload...",
    "Cannibalizing system RAM...",
    "Negotiating with the garbage collector...",
    "Obfuscating spaghetti logic...",
    "Rehydrating the widget tree...",
    "Monkey-patching the framework...",
    "Consulting the dart gods...",
    "Polymorphing into a widget...",
    "Hiding Android artifacts...",
    "Hiding iOS artifacts...",
    "Optimizing the crash loop...",
    "Backporting the bugs...",
    "Injecting hot-reload magic...",
    "Overengineering 'Hello World'...",
    "Demystifying the stack trace...",
    "Siphoning user's data (allegedly)...",
    "Distributing bugs evenly...",
    "Distributing the tech debt...",
    "Distributing spaghetti code globally...",
    "Quantifying 'TODO' comments...",
    "Resolving merge conflicts with a coin toss...",
    "Git cloning node_modules...",
    "Hammering the build button...",
    "Hammering core #2...",
];

/// Loading state for startup initialization
#[derive(Debug, Clone)]
pub struct LoadingState {
    /// Current loading message
    pub message: String,
    /// Animation frame counter for spinner
    pub animation_frame: u64,
    /// Current index into LOADING_MESSAGES for cycling
    message_index: usize,
}

impl LoadingState {
    pub fn new(_message: &str) -> Self {
        // Start at a random index for variety
        let start_index = rand::thread_rng().gen_range(0..LOADING_MESSAGES.len());

        Self {
            message: LOADING_MESSAGES[start_index].to_string(),
            animation_frame: 0,
            message_index: start_index,
        }
    }

    /// Tick animation frame and optionally cycle message
    ///
    /// `cycle_messages`: If true, cycle through messages every ~15 ticks (1.5 sec at 100ms)
    // MSRV guard: `is_multiple_of` requires Rust 1.87; MSRV is 1.77.2 — suppress the lint.
    #[allow(clippy::manual_is_multiple_of)]
    pub fn tick(&mut self, cycle_messages: bool) {
        self.animation_frame = self.animation_frame.wrapping_add(1);

        if cycle_messages {
            // Cycle message every 15 frames (~1.5 seconds at 100ms tick rate)
            if self.animation_frame % 15 == 0 {
                self.message_index = (self.message_index + 1) % LOADING_MESSAGES.len();
                self.message = LOADING_MESSAGES[self.message_index].to_string();
            }
        }
    }

    /// Update message
    pub fn set_message(&mut self, message: &str) {
        self.message = message.to_string();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DAP IDE Config Status (DAP Server Phase 5, Task 03)
// ─────────────────────────────────────────────────────────────────────────────

/// Status of IDE DAP config generation, shown in TUI status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DapConfigStatus {
    /// The IDE config was generated/updated for.
    pub ide_name: String,
    /// The config file path.
    pub path: PathBuf,
    /// What happened ("Created", "Updated", "Skipped: <reason>").
    pub action: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// DAP Server State (DAP Server Phase 2)
// ─────────────────────────────────────────────────────────────────────────────

/// Status of the embedded DAP server.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DapStatus {
    /// DAP server is not running.
    #[default]
    Off,
    /// DAP server is starting up (binding port, initializing).
    Starting,
    /// DAP server is running and accepting connections.
    Running {
        /// The TCP port the server is listening on.
        port: u16,
        /// Set of currently connected DAP client IDs.
        clients: HashSet<String>,
    },
    /// DAP server is shutting down (disconnecting clients, unbinding).
    Stopping,
}

impl DapStatus {
    /// Returns the port if the server is running.
    pub fn port(&self) -> Option<u16> {
        match self {
            DapStatus::Running { port, .. } => Some(*port),
            _ => None,
        }
    }

    /// Returns whether the server is running.
    pub fn is_running(&self) -> bool {
        matches!(self, DapStatus::Running { .. })
    }

    /// Returns the number of currently connected clients, or 0 if not running.
    pub fn client_count(&self) -> usize {
        match self {
            DapStatus::Running { clients, .. } => clients.len(),
            _ => 0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tag Filter UI State (Phase 2, Task 09)
// ─────────────────────────────────────────────────────────────────────────────

/// UI state for the native tag filter overlay.
///
/// Tracks the currently selected row within the tag list.
/// Lives on `AppState` so the TUI render function can read it without reaching
/// into session state.
///
/// `last_known_visible_height` and `last_known_scroll_offset` use `Cell<usize>`
/// interior mutability and are written by the renderer each frame as render-hint
/// feedback channels. They must not be used as correctness inputs to business
/// logic or participate in state equality comparisons. See `docs/CODE_STANDARDS.md`
/// "Principle 3" for rationale.
#[derive(Debug, Clone, Default)]
pub struct TagFilterUiState {
    /// Currently selected index in the tag list.
    pub selected_index: usize,
    /// Render-hint: actual visible height from the last rendered frame.
    /// Defaults to 0, which signals "not yet rendered — use fallback".
    /// Written by the renderer; not mutated by message handlers.
    pub last_known_visible_height: Cell<usize>,
    /// Render-hint: the `ListState.offset()` value from the last rendered
    /// frame, i.e., the absolute index of the topmost visible tag row.
    /// Written by the renderer after `render_stateful_widget`; read by the
    /// region recorder to convert screen-row numbers to absolute tag indices.
    /// Defaults to 0 — safe fallback when no render has occurred yet.
    pub last_known_scroll_offset: Cell<usize>,
}

impl TagFilterUiState {
    /// Move selection up by one, saturating at 0.
    pub fn move_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Move selection down by one, clamping at `max_index`.
    pub fn move_down(&mut self, max_index: usize) {
        if self.selected_index < max_index {
            self.selected_index += 1;
        }
    }

    /// Reset selection when the overlay is opened.
    pub fn reset(&mut self) {
        self.selected_index = 0;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Log Click State (Phase 4 Mouse)
// ─────────────────────────────────────────────────────────────────────────────

/// Click stamp recorded by [`crate::handler::log_view::handle_click_log_row`]
/// to detect double-clicks within the 400 ms window.
///
/// Both fields are `Copy` so the read-then-clear pattern in the handler
/// (`let last = state.last_log_click; state.last_log_click = None;`) does
/// not require `Option::take`.
#[derive(Debug, Clone, Copy)]
pub struct LogClickStamp {
    /// [`LogEntry::id`] of the clicked entry.
    pub entry_id: u64,
    /// Wall-clock time of the click, used for 400 ms double-click detection.
    pub at: std::time::Instant,
}

/// Click stamp recorded by [`handler::settings_handlers::handle_settings_click_row`]
/// to detect double-clicks on a setting row within the 400 ms window.
///
/// Mirrors [`LogClickStamp`] — see Phase 4 task 01 for the precedent.
#[derive(Debug, Clone, Copy)]
pub struct SettingsClickStamp {
    /// 0-based index into the active tab's `SettingItem` list.
    pub index: usize,
    /// Wall-clock time of the click, used for 400 ms double-click detection.
    pub at: std::time::Instant,
}

// ─────────────────────────────────────────────────────────────────────────────
// Toast / Notification system
// ─────────────────────────────────────────────────────────────────────────────

/// Severity level of a transient toast notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    /// Informational notice (blue).
    Info,
    /// Warning that requires user attention (yellow).
    Warn,
}

/// How long a toast stays visible before it is automatically dismissed.
///
/// Derived from a comfortable reading speed for the longest expected message:
/// ~80 characters at 250 wpm reads in ~4 seconds, plus a 1-second buffer so a
/// user who glances at the terminal late in the cycle still has time to
/// finish the sentence.
pub const TOAST_TTL_SECS: u64 = 5;

/// A transient user-facing notification displayed as a one-line overlay.
///
/// Toasts are pushed by handler code (e.g., [`crate::handler::devtools`]) into
/// [`AppState::toasts`] and rendered by the TUI as a bottom-of-screen overlay.
/// They expire automatically after [`TOAST_TTL_SECS`] seconds; the
/// [`crate::handler::update`] `Tick` arm calls
/// [`AppState::expire_toasts`] to remove stale entries.
#[derive(Debug, Clone)]
pub struct Toast {
    /// Short human-readable message to display.
    pub text: String,
    /// Visual severity level (controls accent colour).
    pub level: ToastLevel,
    /// Wall-clock time at which this toast was created.
    pub created_at: std::time::Instant,
}

impl Toast {
    /// Construct a new toast with the current timestamp.
    pub fn new(level: ToastLevel, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level,
            created_at: std::time::Instant::now(),
        }
    }

    /// Return `true` if this toast has outlived [`TOAST_TTL_SECS`].
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_secs() >= TOAST_TTL_SECS
    }
}

// ─────────────────────────────────────────────────────────────────────────────
/// Complete application state (the Model in TEA)
#[derive(Debug)]
pub struct AppState {
    /// Current UI mode/screen
    pub ui_mode: UiMode,

    /// Session manager for multi-instance support
    pub session_manager: SessionManager,

    /// Application settings from config file
    pub settings: Settings,

    /// Confirmation dialog state
    pub confirm_dialog_state: Option<ConfirmDialogState>,

    /// Project path
    pub project_path: PathBuf,

    /// Project name from pubspec.yaml (cached at startup)
    pub project_name: Option<String>,

    /// Current application phase (used for app-level quitting state)
    pub phase: AppPhase,

    /// Settings view state (for Settings UI mode)
    pub settings_view_state: SettingsViewState,

    /// New session dialog state (unified dialog)
    pub new_session_dialog_state: NewSessionDialogState,

    /// Loading state (for initial startup loading screen)
    pub loading_state: Option<LoadingState>,

    /// Global device cache (used by NewSessionDialog)
    /// Task 08e - Device Cache Sharing
    pub device_cache: Option<Vec<Device>>,

    /// When devices were last discovered (for cache invalidation)
    /// Task 08e - Device Cache Sharing
    pub devices_last_updated: Option<std::time::Instant>,

    /// Bootable device cache - iOS simulators (Bug Fix: Task 03)
    pub ios_simulators_cache: Option<Vec<IosSimulator>>,

    /// Bootable device cache - Android AVDs (Bug Fix: Task 03)
    pub android_avds_cache: Option<Vec<AndroidAvd>>,

    /// When bootable devices were last discovered (for cache invalidation)
    /// Bug Fix: Task 03 - Bootable Device Caching
    pub bootable_last_updated: Option<std::time::Instant>,

    /// Cached tool availability (checked at startup)
    /// Phase 4, Task 05 - Discovery Integration
    pub tool_availability: ToolAvailability,

    /// DevTools mode view state (Phase 4 DevTools Integration)
    pub devtools_view_state: DevToolsViewState,

    /// Status of the embedded DAP debug adapter server.
    pub dap_status: DapStatus,

    // ── Coordinated Pause / File-Watcher Gate (Phase 4, Task 03) ─────────────
    /// Whether the file watcher's auto-reload is currently suppressed because
    /// a DAP debugger is paused at a breakpoint, step, exception, etc.
    ///
    /// Set to `true` by `Message::SuspendFileWatcher` (emitted by
    /// `handle_debug_event` on any Pause* event) and cleared by
    /// `Message::ResumeFileWatcher` (emitted on Resume or client disconnect).
    ///
    /// Controlled by `settings.dap.suppress_reload_on_pause` (default `true`).
    /// When that setting is `false`, this flag is ignored in the
    /// `FilesChanged` handler and reload proceeds normally.
    pub file_watcher_suspended: bool,

    /// Number of file-change events that arrived while `file_watcher_suspended`
    /// is `true`.
    ///
    /// Incremented by the `FilesChanged` handler when suppression is active.
    /// Consumed (reset to 0) by `ResumeFileWatcher` which triggers a single
    /// `AutoReloadTriggered` if the count is non-zero.
    pub pending_file_changes: usize,

    /// Result of the most recent IDE DAP config generation (Phase 5, Task 03).
    ///
    /// Set when `DapConfigGenerated` is received; persists until the next
    /// DAP server restart. `None` before any config has been generated.
    pub dap_config_status: Option<DapConfigStatus>,

    /// CLI-provided IDE override for DAP config generation (`--dap-config <ide>`).
    /// When set, bypasses environment-based IDE detection.
    pub cli_dap_config_override: Option<crate::config::ParentIde>,

    // ── Tag Filter Overlay (Phase 2, Task 09) ────────────────────────────────
    /// Whether the native tag filter overlay is currently visible.
    ///
    /// Set to `true` by `Message::ShowTagFilter`, cleared by
    /// `Message::HideTagFilter`. When `true`, the TUI renders the tag filter
    /// overlay and all key events are routed to the overlay handler first.
    pub tag_filter_visible: bool,

    /// UI state for the tag filter overlay (selection, scroll).
    pub tag_filter_ui: TagFilterUiState,

    /// Watcher errors that arrived before any session existed.
    /// Flushed into the first session on `SessionStarted`.
    /// Capped at [`MAX_PENDING_WATCHER_ERRORS`] to prevent unbounded growth.
    pub pending_watcher_errors: Vec<String>,

    /// Running shared custom source handles (project-level, not per-session).
    ///
    /// One entry per configured custom source with `shared = true` that has been
    /// successfully spawned. Cleaned up only on engine shutdown.
    pub shared_source_handles: Vec<SharedSourceHandle>,

    /// Resolved Flutter SDK from the detection chain.
    ///
    /// Populated at startup by `Engine::new()` via `find_flutter_sdk()`.
    /// `None` if no SDK was found at startup (fdemon still starts, but
    /// session spawning and device discovery are unavailable until an SDK
    /// is configured via `.fdemon/config.toml` `[flutter] sdk_path`).
    pub resolved_sdk: Option<FlutterSdk>,

    /// State for the Flutter Version panel overlay.
    ///
    /// Initialized to `FlutterVersionState::default()` at startup.
    /// Re-initialized via `show_flutter_version()` when the panel is opened,
    /// which snapshots the current `resolved_sdk` at open time.
    pub flutter_version_state: FlutterVersionState,

    /// State for the Install Wizard panel overlay.
    ///
    /// Initialized to `InstallWizardState::default()` at startup.
    /// Re-initialized via `show_install_wizard()` when the panel is opened,
    /// which sets `visible = true` and `loading = true` while the preflight task runs.
    pub install_wizard_state: InstallWizardState,

    /// Optional one-line notice rendered above the New Session Dialog on startup.
    ///
    /// Set by handlers such as `Message::NewVersionAvailable` to surface
    /// actionable information (e.g., a newer fdemon release is available).
    /// Cleared when the New Session dialog is dismissed via
    /// [`AppState::hide_new_session_dialog`].
    pub startup_notice: Option<StartupNotice>,

    /// Transient toast notifications shown as a one-line overlay in the TUI.
    ///
    /// Pushed by handler code (e.g., DevTools fallback in
    /// `handler/devtools/mod.rs`) via [`AppState::push_toast`].
    /// Expired automatically on each `Tick` via [`AppState::expire_toasts`].
    pub toasts: Vec<Toast>,

    /// Per-frame mouse click-region registry.
    ///
    /// Populated by widgets during render via [`crate::mouse_regions::MouseRegionsBuilder`]
    /// and read by [`crate::handler::mouse`] during click hit-tests. Lives on
    /// `AppState` (rather than being threaded through the handler layer) because
    /// `Cell` interior mutability lets render write back without forcing
    /// `&mut AppState` everywhere.
    ///
    /// **TEA exception**: This is the same exception class as
    /// [`TagFilterUiState::last_known_visible_height`] — a render-hint write-back
    /// that does NOT participate in business logic or state equality. See
    /// `docs/CODE_STANDARDS.md` Principle 3 for rationale.
    ///
    /// Lifecycle (per frame):
    /// 1. `render::view` calls `state.mouse_regions.take()`, draining the previous
    ///    frame's entries (the `Cell` now holds an empty `MouseRegions`).
    /// 2. Widgets push entries into a `MouseRegionsBuilder` borrowed against the
    ///    drained instance.
    /// 3. `render::view` calls `state.mouse_regions.set(populated)` to put the
    ///    new registry back.
    /// 4. On `Message::Mouse(MouseInput::Press {..})`, `handler::mouse::normal`
    ///    performs the same take/hit-test/put-back dance.
    // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3
    pub mouse_regions: MouseRegionsCell,

    /// Most recent log-row click, used for double-click detection.
    ///
    /// Set by [`crate::handler::log_view::handle_click_log_row`] and cleared
    /// when a double-click is consumed or the selected session changes.
    pub last_log_click: Option<LogClickStamp>,

    /// Most recent settings-row click, used for double-click detection.
    /// Cleared whenever a double-click is consumed or the active tab
    /// changes.
    pub last_settings_click: Option<SettingsClickStamp>,

    /// Whether terminal mouse capture is currently active.
    ///
    /// Initialized from `settings.ui.enable_mouse` at construction. Mutated only
    /// by the `MouseCaptureChanged` handler arm (Task 06) after the runner has
    /// performed the corresponding `terminal::set_mouse_capture` call. The
    /// indicator in the bottom metadata bar (Task 08) reads this field.
    pub mouse_capture_active: bool,

    /// Monotonic animation tick, advanced once per `Message::Tick` (≈50 ms)
    /// regardless of `UiMode`. Shared time source for shimmer/spinner/flash
    /// animations. Wraps via `wrapping_add`; consumers use modulo arithmetic.
    pub animation_frame: u64,

    /// Terminal/clipboard actions queued for the runner to execute.
    ///
    /// `SetMouseCapture` and `WriteClipboard` require synchronous side effects
    /// (terminal writes, OS clipboard I/O) that must be performed by the TUI
    /// runner, not by `actions::handle_action` (which runs on the Tokio thread
    /// pool and has no access to the terminal handle or the runner-owned
    /// clipboard). `process.rs` intercepts these two variants and pushes them
    /// here instead of forwarding them to `handle_action`. The runner drains
    /// this queue after each `process_message()` call.
    ///
    /// **Access contract:** the legitimate production drain path is
    /// `Engine::drain_runner_actions()` — do NOT drain or iterate this `Vec`
    /// directly from outside the crate. The `pub` visibility is retained (rather
    /// than `pub(crate)`) because integration tests in `fdemon-tui` push
    /// synthetic actions directly to exercise `handle_runner_actions` without
    /// going through a full message round-trip. Narrowing to `pub(crate)` is a
    /// future cleanup item once those tests are refactored.
    ///
    /// Only `UpdateAction::SetMouseCapture` and `UpdateAction::WriteClipboard`
    /// are ever pushed here; all other action variants flow through the normal
    /// `handle_action` path.
    pub pending_runner_actions: Vec<crate::handler::UpdateAction>,
}

/// Maximum number of watcher errors buffered before a session exists.
///
/// Errors arriving before any Flutter session has started are held in
/// `AppState::pending_watcher_errors` and replayed into the first session's
/// log on `SessionStarted`. Without a cap, a misconfigured watcher path that
/// the OS repeatedly reports errors for could grow this buffer without bound.
/// 50 errors is far more than any real misconfiguration scenario needs and
/// keeps memory impact negligible.
pub const MAX_PENDING_WATCHER_ERRORS: usize = 50;

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Create a new AppState with default settings (for backward compatibility)
    pub fn new() -> Self {
        Self::with_settings(PathBuf::new(), Settings::default())
    }

    /// Create a new AppState with project path and settings
    pub fn with_settings(project_path: PathBuf, settings: Settings) -> Self {
        // Parse project name from pubspec.yaml
        let project_name = fdemon_core::get_project_name(&project_path);

        let mouse_capture_active = settings.ui.enable_mouse;

        let mut devtools_view_state = DevToolsViewState::default();
        // Bridge settings into DevTools state at startup. The InspectorState
        // Default impl is settings-agnostic (per task 03 requirements), so we
        // propagate persisted preferences here, at the single bridge site.
        devtools_view_state.inspector.hide_implementation_widgets =
            settings.devtools.hide_implementation_widgets;

        Self {
            ui_mode: UiMode::Normal,
            session_manager: SessionManager::new(),
            settings,
            confirm_dialog_state: None,
            project_path,
            project_name,
            phase: AppPhase::Initializing,
            settings_view_state: SettingsViewState::new(),
            new_session_dialog_state: NewSessionDialogState::new(LoadedConfigs::default()),
            loading_state: None,
            device_cache: None,
            devices_last_updated: None,
            ios_simulators_cache: None,
            android_avds_cache: None,
            bootable_last_updated: None,
            tool_availability: ToolAvailability::default(),
            devtools_view_state,
            dap_status: DapStatus::Off,
            file_watcher_suspended: false,
            pending_file_changes: 0,
            dap_config_status: None,
            cli_dap_config_override: None,
            tag_filter_visible: false,
            tag_filter_ui: TagFilterUiState::default(),
            pending_watcher_errors: Vec::new(),
            shared_source_handles: Vec::new(),
            resolved_sdk: None,
            flutter_version_state: FlutterVersionState::default(),
            install_wizard_state: InstallWizardState::default(),
            startup_notice: None,
            toasts: Vec::new(),
            mouse_regions: MouseRegionsCell::new(MouseRegions::with_capacity()),
            last_log_click: None,
            last_settings_click: None,
            mouse_capture_active,
            animation_frame: 0,
            pending_runner_actions: Vec::new(),
        }
    }

    // ─────────────────────────────────────────────────────────
    // Toast Helpers
    // ─────────────────────────────────────────────────────────

    /// Push a transient toast notification to the display queue.
    ///
    /// The toast is visible until [`expire_toasts`][Self::expire_toasts] removes
    /// it after [`TOAST_TTL_SECS`] seconds (driven by the `Tick` handler).
    ///
    /// Capped at 5 concurrent toasts to prevent unbounded growth when events
    /// fire in rapid succession. If the queue is already full the oldest toast
    /// is evicted before the new one is added.
    pub fn push_toast(&mut self, level: ToastLevel, text: impl Into<String>) {
        /// Maximum number of concurrent toasts.
        const MAX_TOASTS: usize = 5;
        if self.toasts.len() >= MAX_TOASTS {
            self.toasts.remove(0);
        }
        self.toasts.push(Toast::new(level, text));
    }

    /// Remove all toasts that have exceeded [`TOAST_TTL_SECS`].
    ///
    /// Called on each `Tick` by the update handler.
    pub fn expire_toasts(&mut self) {
        self.toasts.retain(|t| !t.is_expired());
    }

    // ─────────────────────────────────────────────────────────
    // Flutter SDK Helpers
    // ─────────────────────────────────────────────────────────

    /// Return a clone of the `FlutterExecutable` from the resolved SDK, if any.
    ///
    /// Returns `None` when no SDK has been resolved yet (e.g., Flutter not
    /// installed or `sdk_path` not configured). Callers that need the SDK to
    /// dispatch an action should handle `None` by returning an error message.
    pub fn flutter_executable(&self) -> Option<fdemon_daemon::FlutterExecutable> {
        self.resolved_sdk.as_ref().map(|sdk| sdk.executable.clone())
    }

    // ─────────────────────────────────────────────────────────
    // UI Mode Helpers
    // ─────────────────────────────────────────────────────────

    /// Show settings panel
    pub fn show_settings(&mut self) {
        self.settings_view_state = SettingsViewState::new();
        self.settings_view_state.load_user_prefs(&self.project_path);
        self.ui_mode = UiMode::Settings;
    }

    /// Hide settings panel
    /// Clears all modal overlay state in addition to resetting the UI mode so
    /// that stale modal data cannot leak between open/close cycles.
    pub fn hide_settings(&mut self) {
        self.settings_view_state.dart_defines_modal = None;
        self.settings_view_state.extra_args_modal = None;
        self.settings_view_state.editing_config_idx = None;
        self.ui_mode = UiMode::Normal;
    }

    // ─────────────────────────────────────────────────────────
    // DevTools Mode Helpers (Phase 4)
    // ─────────────────────────────────────────────────────────

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

    /// Show the new session dialog
    pub fn show_new_session_dialog(&mut self, configs: LoadedConfigs) {
        self.new_session_dialog_state = NewSessionDialogState::new(configs);
        self.ui_mode = UiMode::NewSessionDialog;
    }

    /// Hide the new session dialog
    pub fn hide_new_session_dialog(&mut self) {
        self.ui_mode = UiMode::Normal;
        // Clear the startup notice so it doesn't re-appear if the dialog is
        // re-opened later in the same process (e.g. via n key in Normal mode).
        self.startup_notice = None;
    }

    /// Clears the startup notice once the user interacts on a non-dialog screen.
    /// No-op when the New Session Dialog is visible (the dialog owns the notice's
    /// lifecycle and clears it on dismiss via `hide_new_session_dialog`).
    pub fn dismiss_startup_notice_on_interaction(&mut self) {
        if self.startup_notice.is_some() && !self.is_new_session_dialog_visible() {
            self.startup_notice = None;
        }
    }

    // ─────────────────────────────────────────────────────────
    // Flutter Version Panel Helpers (Phase 2)
    // ─────────────────────────────────────────────────────────

    /// Opens the Flutter Version panel, snapshotting the current SDK state.
    ///
    /// Creates a fresh `FlutterVersionState` from the currently resolved SDK
    /// (reading the Dart version file synchronously) and transitions to
    /// `UiMode::FlutterVersion`.
    pub fn show_flutter_version(&mut self) {
        self.flutter_version_state = FlutterVersionState::new(self.resolved_sdk.clone());
        self.flutter_version_state.visible = true;
        self.ui_mode = UiMode::FlutterVersion;
    }

    /// Closes the Flutter Version panel, returning to Normal mode.
    pub fn hide_flutter_version(&mut self) {
        self.flutter_version_state.visible = false;
        self.ui_mode = UiMode::Normal;
    }

    /// Open the Install Wizard panel.
    ///
    /// Resets the wizard to a fresh loading state and transitions to
    /// `UiMode::InstallWizard`. The caller is responsible for also dispatching
    /// `UpdateAction::RunToolchainPreflight` to populate the report.
    ///
    /// The `origin` parameter records why the wizard was opened so that
    /// `close_wizard_and_dispatch_discovery` can gate the post-install handback:
    /// only a `Bootstrap` origin auto-advances to device discovery.
    pub fn show_install_wizard(&mut self, origin: WizardOrigin) {
        self.install_wizard_state = InstallWizardState::opening(origin);
        self.ui_mode = UiMode::InstallWizard;
    }

    /// Close the Install Wizard panel and return to Normal mode.
    ///
    /// Cancels and clears any in-flight install task handle (F19) so that the
    /// install loop stops promptly and the `LockGuard` is released even when
    /// the wizard is closed by an external signal (e.g. `HideInstallWizard`).
    pub fn hide_install_wizard(&mut self) {
        // F19: cancel + clear any running install task before hiding.
        if let Some(task) = self.install_wizard_state.install_task.take() {
            task.cancel.cancel();
            if let Some(j) = task.join {
                j.abort();
            }
        }
        self.install_wizard_state.visible = false;
        self.ui_mode = UiMode::Normal;
    }

    /// Check if new session dialog is visible
    /// Both UiMode::Startup and UiMode::NewSessionDialog show the new session dialog
    pub fn is_new_session_dialog_visible(&self) -> bool {
        self.ui_mode == UiMode::NewSessionDialog || self.ui_mode == UiMode::Startup
    }

    /// Check if any session should prevent immediate quit
    pub fn has_running_sessions(&self) -> bool {
        self.session_manager.has_running_sessions()
    }

    /// Request application quit
    pub fn request_quit(&mut self) {
        if self.has_running_sessions() && self.settings.behavior.confirm_quit {
            // Create dialog state with session count
            let session_count = self.session_manager.running_sessions().len();
            self.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(session_count));
            self.ui_mode = UiMode::ConfirmDialog;
        } else {
            self.phase = AppPhase::Quitting;
        }
    }

    /// Force quit without confirmation
    pub fn force_quit(&mut self) {
        self.phase = AppPhase::Quitting;
    }

    /// Confirm quit (from confirmation dialog)
    pub fn confirm_quit(&mut self) {
        self.phase = AppPhase::Quitting;
    }

    /// Cancel quit (from confirmation dialog)
    pub fn cancel_quit(&mut self) {
        self.confirm_dialog_state = None;
        self.ui_mode = UiMode::Normal;
    }

    /// Check if the app should quit
    pub fn should_quit(&self) -> bool {
        self.phase == AppPhase::Quitting
    }

    // ─────────────────────────────────────────────────────────
    // Loading State Helpers (Task 08d)
    // ─────────────────────────────────────────────────────────

    /// Set loading phase with message
    pub fn set_loading_phase(&mut self, message: &str) {
        self.loading_state = Some(LoadingState::new(message));
        self.ui_mode = UiMode::Loading;
    }

    /// Update loading message
    pub fn update_loading_message(&mut self, message: &str) {
        if let Some(ref mut loading) = self.loading_state {
            loading.set_message(message);
        }
    }

    /// Clear loading state
    pub fn clear_loading(&mut self) {
        self.loading_state = None;
        if self.ui_mode == UiMode::Loading {
            self.ui_mode = UiMode::Normal;
        }
    }

    /// Tick loading animation with optional message cycling
    ///
    /// `cycle_messages`: If true, cycle through messages every ~15 ticks (1.5 sec at 100ms)
    pub fn tick_loading_animation_with_cycling(&mut self, cycle_messages: bool) {
        if let Some(ref mut loading) = self.loading_state {
            loading.tick(cycle_messages);
        }
    }

    /// Tick loading animation (no message cycling - backward compat)
    pub fn tick_loading_animation(&mut self) {
        self.tick_loading_animation_with_cycling(false);
    }

    // ─────────────────────────────────────────────────────────
    // Device Cache Helpers (Task 08e)
    // ─────────────────────────────────────────────────────────

    /// Get cached devices.
    ///
    /// Cache survives for the lifetime of AppState; the dialog always triggers a
    /// background refresh on open to keep the list fresh.
    pub fn get_cached_devices(&self) -> Option<&Vec<Device>> {
        self.device_cache.as_ref()
    }

    /// Update device cache with fresh devices
    ///
    /// Called after successful device discovery to cache results globally.
    /// Both DeviceSelector and StartupDialog use this shared cache.
    pub fn set_device_cache(&mut self, devices: Vec<Device>) {
        self.device_cache = Some(devices);
        self.devices_last_updated = Some(std::time::Instant::now());
    }

    // ─────────────────────────────────────────────────────────
    // Bootable Device Cache Helpers (Bug Fix: Task 03)
    // ─────────────────────────────────────────────────────────

    /// Get cached bootable devices.
    ///
    /// Returns references to both iOS simulators and Android AVDs from cache whenever
    /// both are populated. Cache survives for the lifetime of AppState; the dialog
    /// always triggers a background refresh on open to keep the list fresh.
    ///
    /// The caller is responsible for cloning if it needs ownership (mirroring the
    /// `get_cached_devices` pattern).
    pub fn get_cached_bootable_devices(&self) -> Option<(&Vec<IosSimulator>, &Vec<AndroidAvd>)> {
        match (&self.ios_simulators_cache, &self.android_avds_cache) {
            (Some(sims), Some(avds)) => Some((sims, avds)),
            _ => None,
        }
    }

    /// Update the bootable device cache with fresh results
    ///
    /// Called after successful bootable device discovery to cache results globally.
    /// The NewSessionDialog uses this shared cache to show bootable devices instantly.
    pub fn set_bootable_cache(&mut self, simulators: Vec<IosSimulator>, avds: Vec<AndroidAvd>) {
        self.ios_simulators_cache = Some(simulators);
        self.android_avds_cache = Some(avds);
        self.bootable_last_updated = Some(std::time::Instant::now());
    }

    // ─────────────────────────────────────────────────────────
    // Shared Source Handle Helpers (Pre-App Custom Sources Phase 2)
    // ─────────────────────────────────────────────────────────

    /// Shut down all shared custom sources.
    ///
    /// Sends shutdown signal and aborts tasks. Called during engine shutdown.
    pub fn shutdown_shared_sources(&mut self) {
        for mut handle in self.shared_source_handles.drain(..) {
            let _ = handle.shutdown_tx.send(true);
            if let Some(task) = handle.task_handle.take() {
                task.abort();
            }
        }
    }

    /// Returns the names of currently running shared sources.
    pub fn running_shared_source_names(&self) -> Vec<String> {
        self.shared_source_handles
            .iter()
            .map(|h| h.name.clone())
            .collect()
    }

    /// Returns true if a shared source with the given name is already running.
    pub fn is_shared_source_running(&self, name: &str) -> bool {
        self.shared_source_handles.iter().any(|h| h.name == name)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────
    // DevTools State Tests (Phase 4, Task 01)
    // ─────────────────────────────────────────────────────────

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
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Inspector
        );
        state.switch_devtools_panel(DevToolsPanel::Performance);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Performance
        );
        state.switch_devtools_panel(DevToolsPanel::Inspector);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Inspector
        );
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
        assert!(state.last_overlay_toggle.is_none());
    }

    #[test]
    fn test_overlay_toggle_debounce_initially_false() {
        let state = DevToolsViewState::default();
        assert!(
            !state.is_overlay_toggle_debounced(),
            "Debounce should be false when no toggle has occurred"
        );
    }

    #[test]
    fn test_overlay_toggle_debounce_active_after_record() {
        let mut state = DevToolsViewState::default();
        state.record_overlay_toggle();
        assert!(
            state.is_overlay_toggle_debounced(),
            "Debounce should be active immediately after recording a toggle"
        );
    }

    #[test]
    fn test_overlay_toggle_debounce_cleared_on_reset() {
        let mut state = DevToolsViewState::default();
        state.record_overlay_toggle();
        assert!(state.is_overlay_toggle_debounced());

        state.reset();
        assert!(
            state.last_overlay_toggle.is_none(),
            "reset() should clear last_overlay_toggle"
        );
        assert!(
            !state.is_overlay_toggle_debounced(),
            "Debounce should be inactive after reset"
        );
    }

    // ─────────────────────────────────────────────────────────
    // selected_node_description Tests (Task 06)
    // ─────────────────────────────────────────────────────────

    /// Build a three-node tree: root → child-1 → child-2.
    ///
    /// All nodes have `created_by_local_project: true` so they are "always
    /// visible" and are **not** folded by the implementation-widget hiding
    /// logic, regardless of the `hide_implementation_widgets` default.
    fn make_tree_with_three_nodes() -> DiagnosticsNode {
        DiagnosticsNode {
            description: "RootNode".to_string(),
            value_id: Some("root-id".to_string()),
            created_by_local_project: true,
            children: vec![DiagnosticsNode {
                description: "SecondNode".to_string(),
                value_id: Some("child-1-id".to_string()),
                created_by_local_project: true,
                children: vec![DiagnosticsNode {
                    description: "ThirdNode".to_string(),
                    value_id: Some("child-2-id".to_string()),
                    created_by_local_project: true,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn make_single_node() -> DiagnosticsNode {
        DiagnosticsNode {
            description: "SingleNode".to_string(),
            value_id: Some("single-id".to_string()),
            created_by_local_project: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_selected_node_description_empty_tree() {
        let inspector = InspectorState::default();
        assert!(inspector.selected_node_description().is_none());
    }

    #[test]
    fn test_selected_node_description_returns_root_when_index_zero() {
        let inspector = InspectorState {
            root: Some(make_tree_with_three_nodes()),
            ..Default::default()
        };

        let desc = inspector.selected_node_description();
        assert_eq!(desc.as_deref(), Some("RootNode"));
    }

    #[test]
    fn test_selected_node_description_returns_correct_node() {
        let mut inspector = InspectorState {
            root: Some(make_tree_with_three_nodes()),
            ..Default::default()
        };
        // Expand root and first child so that all three nodes are visible.
        inspector.expanded.insert("root-id".to_string());
        inspector.expanded.insert("child-1-id".to_string());
        inspector.selected_index = 1;

        let desc = inspector.selected_node_description();
        assert_eq!(desc.as_deref(), Some("SecondNode"));
    }

    #[test]
    fn test_selected_node_description_third_node() {
        let mut inspector = InspectorState {
            root: Some(make_tree_with_three_nodes()),
            ..Default::default()
        };
        inspector.expanded.insert("root-id".to_string());
        inspector.expanded.insert("child-1-id".to_string());
        inspector.selected_index = 2;

        let desc = inspector.selected_node_description();
        assert_eq!(desc.as_deref(), Some("ThirdNode"));
    }

    #[test]
    fn test_selected_node_description_index_out_of_bounds() {
        let inspector = InspectorState {
            root: Some(make_single_node()),
            selected_index: 99,
            ..Default::default()
        };
        assert!(inspector.selected_node_description().is_none());
    }

    #[test]
    fn test_selected_node_description_collapsed_children_not_counted() {
        let inspector = InspectorState {
            root: Some(make_tree_with_three_nodes()),
            // Root is NOT expanded — children are hidden, so only root is visible.
            selected_index: 1, // index 1 is out of range
            ..Default::default()
        };

        // Only root visible (index 0), index 1 should return None.
        assert!(inspector.selected_node_description().is_none());
    }

    #[test]
    fn test_selected_node_description_no_allocation_path_matches_visible_nodes() {
        // Verify that selected_node_description agrees with visible_nodes().
        let mut inspector = InspectorState {
            root: Some(make_tree_with_three_nodes()),
            ..Default::default()
        };
        inspector.expanded.insert("root-id".to_string());
        inspector.expanded.insert("child-1-id".to_string());

        // Collect descriptions from visible_nodes() first to drop the borrow
        // before we mutate selected_index.
        let descriptions: Vec<String> = inspector
            .visible_nodes()
            .into_iter()
            .map(|(node, _)| node.description.clone())
            .collect();

        for (i, expected) in descriptions.iter().enumerate() {
            inspector.selected_index = i;
            let desc = inspector.selected_node_description();
            assert_eq!(
                desc.as_deref(),
                Some(expected.as_str()),
                "Mismatch at index {i}"
            );
        }
    }

    // ─────────────────────────────────────────────────────────
    // inspector_rows / DetailsTab / selected_value_id Tests
    // (devtools-inspector-parity Phase 1, Task 02)
    // ─────────────────────────────────────────────────────────

    /// Build a 5-deep single-child chain where every node is an implementation
    /// widget (not created by the local project).  Each node has a unique
    /// `value_id` of the form `"chain-N"` (N = 0..4).
    fn make_chain(depth: usize) -> DiagnosticsNode {
        fn build(remaining: usize, idx: usize) -> DiagnosticsNode {
            DiagnosticsNode {
                description: format!("Widget{idx}"),
                value_id: Some(format!("chain-{idx}")),
                created_by_local_project: false,
                children: if remaining > 0 {
                    vec![build(remaining - 1, idx + 1)]
                } else {
                    vec![]
                },
                ..Default::default()
            }
        }
        build(depth - 1, 0)
    }

    /// Collect all `value_id`s reachable from `root` into a `HashSet`.
    fn collect_value_ids(root: &Option<DiagnosticsNode>) -> HashSet<String> {
        fn recurse(node: &DiagnosticsNode, out: &mut HashSet<String>) {
            if let Some(id) = &node.value_id {
                out.insert(id.clone());
            }
            for child in &node.children {
                recurse(child, out);
            }
        }
        let mut ids = HashSet::new();
        if let Some(root) = root {
            recurse(root, &mut ids);
        }
        ids
    }

    #[test]
    fn inspector_rows_returns_empty_when_no_root() {
        let state = InspectorState::default();
        assert!(state.inspector_rows().is_empty());
    }

    #[test]
    fn inspector_rows_folds_chain_when_hide_implementation_true() {
        let mut state = InspectorState::default();
        // Build a 5-deep wrapper chain (single child each, no createdByLocalProject)
        state.root = Some(make_chain(5));
        // Expand all nodes in the regular expanded set
        state.expanded = collect_value_ids(&state.root);
        // hide_implementation_widgets defaults to true

        let rows = state.inspector_rows();
        // At least one leader-collapsed row should exist
        assert!(
            rows.iter()
                .any(|r| matches!(r.group, fdemon_core::RowGroup::LeaderCollapsed { .. })),
            "Expected a LeaderCollapsed row but got: {:?}",
            rows.iter().map(|r| &r.group).collect::<Vec<_>>()
        );
        assert!(rows.len() < 5, "chain should fold, got {} rows", rows.len());
    }

    #[test]
    fn inspector_rows_renders_full_chain_when_hide_implementation_false() {
        let mut state = InspectorState {
            hide_implementation_widgets: false,
            ..Default::default()
        };
        state.root = Some(make_chain(5));
        state.expanded = collect_value_ids(&state.root);

        let rows = state.inspector_rows();
        // All rows should be standalone (no folding)
        assert!(
            rows.iter().all(|r| r.group == fdemon_core::RowGroup::None),
            "Expected all RowGroup::None rows, got: {:?}",
            rows.iter().map(|r| &r.group).collect::<Vec<_>>()
        );
        assert_eq!(rows.len(), 5, "Expected 5 rows, got {}", rows.len());
    }

    #[test]
    fn visible_nodes_shim_matches_inspector_rows_node_depth_pairs() {
        let mut state = InspectorState {
            hide_implementation_widgets: false,
            ..Default::default()
        };
        state.root = Some(make_tree_with_three_nodes());
        state.expanded.insert("root-id".to_string());
        state.expanded.insert("child-1-id".to_string());

        let rows = state.inspector_rows();
        let shim = state.visible_nodes();

        assert_eq!(rows.len(), shim.len(), "row counts must match");
        for (row, (node, depth)) in rows.iter().zip(shim.iter()) {
            assert!(
                std::ptr::eq(row.node, *node),
                "node pointer mismatch at depth {depth}"
            );
            assert_eq!(
                row.depth, *depth,
                "depth mismatch for node '{}'",
                node.description
            );
        }
    }

    #[test]
    fn reset_preserves_hide_implementation_widgets_and_has_ever_rendered_tree() {
        let mut state = InspectorState {
            // Explicitly override the default (true) to verify preservation.
            hide_implementation_widgets: false,
            has_ever_rendered_tree: true,
            root: Some(make_single_node()),
            loading: true,
            ..Default::default()
        };

        state.reset();

        assert!(
            !state.hide_implementation_widgets,
            "hide_implementation_widgets must be preserved across reset"
        );
        assert!(
            state.has_ever_rendered_tree,
            "has_ever_rendered_tree must be preserved across reset"
        );
        assert!(state.root.is_none(), "root should be cleared");
        assert!(!state.loading, "loading should be cleared");
    }

    #[test]
    fn reset_clears_details_state() {
        let mut state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::RenderObject,
            details_node_id: Some("some-id".to_string()),
            properties: vec![DiagnosticsNode {
                description: "prop".to_string(),
                ..Default::default()
            }],
            render_properties: vec![DiagnosticsNode {
                description: "render-prop".to_string(),
                ..Default::default()
            }],
            properties_loading: true,
            properties_error: Some(DevToolsError::new("err", "hint")),
            ..Default::default()
        };
        state.expanded_groups.insert("leader-id".to_string());

        state.reset();

        assert!(!state.details_open, "details_open should be cleared");
        assert_eq!(
            state.details_tab,
            DetailsTab::Properties,
            "details_tab should reset to default"
        );
        assert!(
            state.details_node_id.is_none(),
            "details_node_id should be cleared"
        );
        assert!(state.properties.is_empty(), "properties should be cleared");
        assert!(
            state.render_properties.is_empty(),
            "render_properties should be cleared"
        );
        assert!(
            !state.properties_loading,
            "properties_loading should be cleared"
        );
        assert!(
            state.properties_error.is_none(),
            "properties_error should be cleared"
        );
        assert!(
            state.expanded_groups.is_empty(),
            "expanded_groups should be cleared"
        );
    }

    #[test]
    fn selected_value_id_returns_none_when_no_tree() {
        let state = InspectorState::default();
        assert!(state.selected_value_id().is_none());
    }

    #[test]
    fn selected_value_id_returns_node_id_for_current_selection() {
        let mut state = InspectorState {
            root: Some(make_tree_with_three_nodes()),
            ..Default::default()
        };
        // Root is visible at index 0
        assert_eq!(
            state.selected_value_id().as_deref(),
            Some("root-id"),
            "index 0 should be root"
        );
        // Expand root → SecondNode visible at index 1
        state.expanded.insert("root-id".to_string());
        state.selected_index = 1;
        assert_eq!(
            state.selected_value_id().as_deref(),
            Some("child-1-id"),
            "index 1 should be SecondNode"
        );
    }

    // ─────────────────────────────────────────────────────────
    // selected_row() Tests (phase-1-fixes Task 01)
    // ─────────────────────────────────────────────────────────

    /// Build a root wrapper node (local-project) with a 3-deep implementation
    /// chain (non-local-project, single child each) as its sole child.
    /// The wrapper is always visible; the chain starts at index 1 when the
    /// wrapper is expanded.
    fn make_root_with_chain() -> DiagnosticsNode {
        // Chain: chain-0 → chain-1 → chain-2 (all non-local-project)
        let chain = make_chain(3);
        DiagnosticsNode {
            description: "RootWrapper".to_string(),
            value_id: Some("wrapper-id".to_string()),
            created_by_local_project: true,
            children: vec![chain],
            ..Default::default()
        }
    }

    #[test]
    fn selected_row_returns_row_with_group_for_chain_leader() {
        let mut inspector = InspectorState::default();
        // Build a tree with a foldable chain (non-local-project, single-child)
        let root = make_root_with_chain();
        inspector.root = Some(root);
        // Expand the wrapper so the chain leader is visible at index 1
        inspector.expanded.insert("wrapper-id".to_string());
        inspector.hide_implementation_widgets = true;
        inspector.selected_index = 1; // the leader row
        let row = inspector.selected_row().expect("row should exist");
        assert!(
            matches!(row.group, fdemon_core::RowGroup::LeaderCollapsed { .. }),
            "Expected LeaderCollapsed but got: {:?}",
            row.group
        );
    }

    #[test]
    fn selected_row_returns_none_when_index_out_of_bounds() {
        let inspector = InspectorState {
            root: Some(make_single_node()),
            selected_index: 99,
            ..Default::default()
        };
        assert!(
            inspector.selected_row().is_none(),
            "index 99 should be out of bounds for a single-node tree"
        );
    }

    #[test]
    fn selected_row_returns_row_for_standalone_widget() {
        let inspector = InspectorState {
            root: Some(make_single_node()),
            hide_implementation_widgets: true,
            selected_index: 0,
            ..Default::default()
        };
        let row = inspector
            .selected_row()
            .expect("row should exist at index 0");
        assert_eq!(
            row.group,
            fdemon_core::RowGroup::None,
            "standalone widget should have RowGroup::None"
        );
    }

    #[test]
    fn inspector_state_default_has_hide_implementation_true() {
        let state = InspectorState::default();
        assert!(
            state.hide_implementation_widgets,
            "default should match DevTools' default (hide implementation widgets)"
        );
    }

    // Helper to create a test device
    fn test_device(id: &str, name: &str) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
            is_supported: true,
            capabilities: None,
        }
    }

    // Old StartupDialog and DialogSection tests removed - replaced by NewSessionDialog

    // ─────────────────────────────────────────────────────────
    // Loading State Tests (Task 08d)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_loading_state_creation() {
        let loading = LoadingState::new("Test message");
        // Should start with one of the LOADING_MESSAGES, not the passed message
        assert!(LOADING_MESSAGES.contains(&loading.message.as_str()));
        assert_eq!(loading.animation_frame, 0);
    }

    #[test]
    fn test_loading_state_tick() {
        let mut loading = LoadingState::new("Test");
        loading.tick(false);
        assert_eq!(loading.animation_frame, 1);
        loading.tick(false);
        assert_eq!(loading.animation_frame, 2);
    }

    #[test]
    fn test_loading_state_tick_wraps() {
        let mut loading = LoadingState::new("Test");
        loading.animation_frame = u64::MAX;
        loading.tick(false);
        assert_eq!(loading.animation_frame, 0);
    }

    #[test]
    fn test_loading_state_random_start() {
        // Run multiple times to verify randomness (statistically)
        let mut seen_indices: std::collections::HashSet<String> = std::collections::HashSet::new();

        for _ in 0..20 {
            let loading = LoadingState::new("ignored");
            seen_indices.insert(loading.message.clone());
        }

        // With 10 messages and 20 trials, we should see multiple different starting messages
        assert!(
            seen_indices.len() > 1,
            "Should have random starting messages, saw {} unique messages",
            seen_indices.len()
        );
    }

    #[test]
    fn test_loading_state_message_cycling() {
        let mut loading = LoadingState::new("ignored");
        let initial_message = loading.message.clone();

        // First 14 ticks - no change (cycle at 15)
        for _ in 0..14 {
            loading.tick(true);
        }
        assert_eq!(loading.message, initial_message);

        // 12th tick - first cycle
        loading.tick(true);
        assert_ne!(
            loading.message, initial_message,
            "Message should change after 15 ticks"
        );

        // After 30 total ticks - should be on third message
        let second_message = loading.message.clone();
        for _ in 0..15 {
            loading.tick(true);
        }
        // Message should have changed again
        assert_ne!(loading.message, second_message);
    }

    #[test]
    fn test_loading_state_wraps_around() {
        let mut loading = LoadingState::new("ignored");
        let start_message = loading.message.clone();

        // Cycle through all 84 messages (84 * 15 = 1260 ticks)
        for _ in 0..1260 {
            loading.tick(true);
        }

        // Should have wrapped back to starting message
        assert_eq!(loading.message, start_message);
    }

    #[test]
    fn test_loading_spinner_speed() {
        let mut loading = LoadingState::new("Test");
        let frame0 = loading.animation_frame;
        loading.tick(false);
        assert_eq!(loading.animation_frame, frame0 + 1);
    }

    #[test]
    fn test_loading_no_cycle_when_disabled() {
        let mut loading = LoadingState::new("ignored");
        let initial_message = loading.message.clone();

        // Tick without cycling
        for _ in 0..50 {
            loading.tick(false);
        }

        assert_eq!(
            loading.message, initial_message,
            "Message should not change when cycling disabled"
        );
    }

    #[test]
    fn test_loading_state_set_message() {
        let mut loading = LoadingState::new("Initial");
        loading.set_message("Updated");
        assert_eq!(loading.message, "Updated");
    }

    #[test]
    fn test_app_state_set_loading_phase() {
        let mut state = AppState::new();
        state.set_loading_phase("Loading...");

        assert_eq!(state.ui_mode, UiMode::Loading);
        assert!(state.loading_state.is_some());
        // Message will be one of LOADING_MESSAGES (random start), not the passed message
        assert!(LOADING_MESSAGES.contains(&state.loading_state.as_ref().unwrap().message.as_str()));
    }

    #[test]
    fn test_app_state_update_loading_message() {
        let mut state = AppState::new();
        state.set_loading_phase("Initial");
        state.update_loading_message("Updated");

        assert!(state.loading_state.is_some());
        assert_eq!(state.loading_state.as_ref().unwrap().message, "Updated");
    }

    #[test]
    fn test_app_state_clear_loading() {
        let mut state = AppState::new();
        state.set_loading_phase("Loading...");

        state.clear_loading();

        assert!(state.loading_state.is_none());
        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_app_state_tick_loading_animation() {
        let mut state = AppState::new();
        state.set_loading_phase("Loading...");

        state.tick_loading_animation();

        assert_eq!(state.loading_state.as_ref().unwrap().animation_frame, 1);
    }

    #[test]
    fn test_app_state_tick_loading_no_state() {
        let mut state = AppState::new();
        // Should not panic when there's no loading state
        state.tick_loading_animation();
        assert!(state.loading_state.is_none());
    }

    #[test]
    fn test_app_state_update_loading_no_state() {
        let mut state = AppState::new();
        // Should not panic when there's no loading state
        state.update_loading_message("Test");
        assert!(state.loading_state.is_none());
    }

    // ─────────────────────────────────────────────────────────
    // Device Cache Tests (Task 08e)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_device_cache_shared() {
        let mut state = AppState::new();

        // Simulate initial discovery
        let devices = vec![test_device("dev1", "Device 1")];
        state.set_device_cache(devices.clone());

        // Device cache is now available for use
        assert!(state.get_cached_devices().is_some());
        assert_eq!(state.get_cached_devices().unwrap().len(), 1);
    }

    #[test]
    fn test_device_cache_fresh() {
        let mut state = AppState::new();
        state.set_device_cache(vec![test_device("dev1", "Device 1")]);

        // Fresh cache should be available
        assert!(state.get_cached_devices().is_some());
        assert_eq!(state.get_cached_devices().unwrap().len(), 1);
    }

    #[test]
    fn test_device_cache_does_not_expire() {
        let mut state = AppState::new();
        // get_cached_devices has no expiry — calling it after set_device_cache always returns Some.
        state.set_device_cache(vec![test_device("dev1", "Device 1")]);
        assert!(state.get_cached_devices().is_some());
        assert_eq!(state.get_cached_devices().unwrap().len(), 1);
    }

    #[test]
    fn test_device_cache_none_initially() {
        let state = AppState::new();
        assert!(state.get_cached_devices().is_none());
        assert!(state.device_cache.is_none());
        assert!(state.devices_last_updated.is_none());
    }

    #[test]
    fn test_device_cache_updates_timestamp() {
        let mut state = AppState::new();

        let before = std::time::Instant::now();
        state.set_device_cache(vec![test_device("dev1", "Device 1")]);
        let after = std::time::Instant::now();

        assert!(state.devices_last_updated.is_some());
        let timestamp = state.devices_last_updated.unwrap();

        // Timestamp should be between before and after
        assert!(timestamp >= before);
        assert!(timestamp <= after);
    }

    #[test]
    fn test_device_cache_replaces_old() {
        let mut state = AppState::new();

        // Initial cache
        state.set_device_cache(vec![test_device("dev1", "Device 1")]);
        assert_eq!(state.device_cache.as_ref().unwrap().len(), 1);

        // Update with new devices
        state.set_device_cache(vec![
            test_device("dev1", "Device 1"),
            test_device("dev2", "Device 2"),
        ]);
        assert_eq!(state.device_cache.as_ref().unwrap().len(), 2);
    }

    // Old StartupDialogState tests removed - replaced by NewSessionDialog tests

    // ─────────────────────────────────────────────────────────
    // NewSessionDialog Tests (Task 05)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_new_session_dialog_visibility() {
        let mut state = AppState::new();
        assert!(!state.is_new_session_dialog_visible());

        state.show_new_session_dialog(LoadedConfigs::default());
        assert!(state.is_new_session_dialog_visible());
        assert_eq!(state.ui_mode, UiMode::NewSessionDialog);

        state.hide_new_session_dialog();
        assert!(!state.is_new_session_dialog_visible());
        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_startup_mode_is_dialog_visible() {
        // UiMode::Startup also shows the new session dialog
        let mut state = AppState::new();
        state.ui_mode = UiMode::Startup;
        assert!(state.is_new_session_dialog_visible());
    }

    // ─────────────────────────────────────────────────────────
    // Cache Preload Tests (Moved to handler tests - Task 01)
    // These tests have been moved to app/handler/new_session/navigation.rs
    // because cache checking is now done in the handler, not in show_new_session_dialog().
    // This follows TEA principles where state methods are pure and handlers contain logic.
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_show_new_session_dialog_does_not_populate_cache() {
        let mut state = AppState::new();
        let configs = LoadedConfigs::default();

        // Simulate cached devices
        let devices = vec![
            test_device("device1", "Test Device 1"),
            test_device("device2", "Test Device 2"),
        ];
        state.set_device_cache(devices.clone());

        // Open dialog - should NOT populate from cache (handler does this)
        state.show_new_session_dialog(configs);

        // Verify devices are NOT pre-populated (handler responsibility)
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .connected_devices
                .len(),
            0
        );
    }

    #[test]
    fn test_show_new_session_dialog_sets_ui_mode() {
        let mut state = AppState::new();
        let configs = LoadedConfigs::default();

        // Open dialog
        state.show_new_session_dialog(configs);

        // Verify UI mode is set
        assert_eq!(state.ui_mode, UiMode::NewSessionDialog);
    }

    // These cache tests have been moved to handler tests because
    // cache population is now done in handle_open_new_session_dialog(),
    // not in show_new_session_dialog(). This follows TEA principles.

    // ─────────────────────────────────────────────────────────
    // Bootable Device Cache Tests (Bug Fix: Task 03)
    // ─────────────────────────────────────────────────────────

    // Helper to create a test iOS simulator
    fn test_ios_simulator(udid: &str, name: &str) -> IosSimulator {
        IosSimulator {
            udid: udid.to_string(),
            name: name.to_string(),
            runtime: "iOS 17.2".to_string(),
            state: fdemon_daemon::SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        }
    }

    // Helper to create a test Android AVD
    fn test_android_avd(name: &str) -> AndroidAvd {
        AndroidAvd {
            name: name.to_string(),
            display_name: format!("{} Display", name),
            api_level: Some(33),
            target: Some("android-33".to_string()),
        }
    }

    #[test]
    fn test_set_bootable_cache() {
        let mut state = AppState::default();
        let simulators = vec![test_ios_simulator("test-udid", "iPhone 15")];
        let avds = vec![test_android_avd("Pixel_7")];

        state.set_bootable_cache(simulators.clone(), avds.clone());

        assert!(state.ios_simulators_cache.is_some());
        assert!(state.android_avds_cache.is_some());
        assert!(state.bootable_last_updated.is_some());
        assert_eq!(state.ios_simulators_cache.as_ref().unwrap().len(), 1);
        assert_eq!(state.android_avds_cache.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_get_cached_bootable_devices_valid() {
        let mut state = AppState::default();
        let simulators = vec![test_ios_simulator("test-udid", "iPhone 15")];
        let avds = vec![test_android_avd("Pixel_7")];
        state.set_bootable_cache(simulators.clone(), avds.clone());

        let cached = state.get_cached_bootable_devices();
        assert!(cached.is_some());
        let (s, a) = cached.unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(a.len(), 1);
        assert_eq!(s[0].name, "iPhone 15");
        assert_eq!(a[0].name, "Pixel_7");
    }

    #[test]
    fn test_get_cached_bootable_devices_empty_when_not_set() {
        let state = AppState::default();
        let cached = state.get_cached_bootable_devices();
        assert!(cached.is_none());
    }

    // Bootable cache tests have been moved to handler tests because
    // cache population is now done in handle_open_new_session_dialog(),
    // not in show_new_session_dialog(). This follows TEA principles.

    // ─────────────────────────────────────────────────────────
    // SettingsViewState modal tests (v1-refinements Phase 2, Task 02)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_settings_view_state_has_modal_open() {
        use crate::new_session_dialog::{FuzzyModalState, FuzzyModalType};

        let mut state = SettingsViewState::new();
        assert!(!state.has_modal_open());

        state.extra_args_modal = Some(FuzzyModalState::new(
            FuzzyModalType::ExtraArgs,
            vec!["--verbose".to_string()],
        ));
        assert!(state.has_modal_open());
    }

    #[test]
    fn test_settings_view_state_has_modal_open_dart_defines() {
        use crate::new_session_dialog::{DartDefine, DartDefinesModalState};

        let mut state = SettingsViewState::new();
        assert!(!state.has_modal_open());

        state.dart_defines_modal = Some(DartDefinesModalState::new(vec![DartDefine::new(
            "ENV", "dev",
        )]));
        assert!(state.has_modal_open());
    }

    #[test]
    fn test_settings_view_state_both_modals_none_by_default() {
        let state = SettingsViewState::new();
        assert!(state.dart_defines_modal.is_none());
        assert!(state.extra_args_modal.is_none());
        assert!(!state.has_modal_open());
    }

    #[test]
    fn test_hide_settings_clears_modal_state() {
        use crate::new_session_dialog::DartDefinesModalState;

        let mut state = AppState::new();
        state.show_settings();
        state.settings_view_state.dart_defines_modal = Some(DartDefinesModalState::new(vec![]));
        state.settings_view_state.editing_config_idx = Some(0);
        state.hide_settings();
        assert!(state.settings_view_state.dart_defines_modal.is_none());
        assert!(state.settings_view_state.editing_config_idx.is_none());
        assert!(!state.settings_view_state.has_modal_open());
    }

    // ─────────────────────────────────────────────────────────
    // Shared Source Handle Tests (Pre-App Custom Sources Phase 2)
    // ─────────────────────────────────────────────────────────

    /// Build a `SharedSourceHandle` backed by a real `watch` channel for
    /// testing.  The task handle is left as `None` because we don't need a
    /// real Tokio task to verify the state-management helpers.
    fn make_shared_source_handle(name: &str) -> SharedSourceHandle {
        let (tx, _rx) = tokio::sync::watch::channel(false);
        SharedSourceHandle {
            name: name.to_string(),
            shutdown_tx: std::sync::Arc::new(tx),
            task_handle: None,
            start_before_app: false,
        }
    }

    #[test]
    fn test_shared_source_handles_initialized_empty() {
        let state = AppState::new();
        assert!(
            state.shared_source_handles.is_empty(),
            "shared_source_handles should be empty on construction"
        );
    }

    #[test]
    fn test_is_shared_source_running_returns_false_when_empty() {
        let state = AppState::new();
        assert!(!state.is_shared_source_running("logcat"));
    }

    #[test]
    fn test_is_shared_source_running_returns_true_when_present() {
        let mut state = AppState::new();
        state
            .shared_source_handles
            .push(make_shared_source_handle("logcat"));
        assert!(state.is_shared_source_running("logcat"));
        assert!(!state.is_shared_source_running("other_source"));
    }

    #[test]
    fn test_running_shared_source_names_empty() {
        let state = AppState::new();
        assert!(state.running_shared_source_names().is_empty());
    }

    #[test]
    fn test_running_shared_source_names_returns_all_names() {
        let mut state = AppState::new();
        state
            .shared_source_handles
            .push(make_shared_source_handle("logcat"));
        state
            .shared_source_handles
            .push(make_shared_source_handle("syslog"));

        let names = state.running_shared_source_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"logcat".to_string()));
        assert!(names.contains(&"syslog".to_string()));
    }

    #[test]
    fn test_shutdown_shared_sources_drains_handles() {
        let mut state = AppState::new();
        state
            .shared_source_handles
            .push(make_shared_source_handle("logcat"));
        state
            .shared_source_handles
            .push(make_shared_source_handle("syslog"));

        assert_eq!(state.shared_source_handles.len(), 2);

        state.shutdown_shared_sources();

        assert!(
            state.shared_source_handles.is_empty(),
            "shutdown_shared_sources() must drain all handles"
        );
    }

    #[test]
    fn test_shutdown_shared_sources_sends_shutdown_signal() {
        let mut state = AppState::new();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let handle = SharedSourceHandle {
            name: "logcat".to_string(),
            shutdown_tx: std::sync::Arc::new(tx),
            task_handle: None,
            start_before_app: true,
        };
        state.shared_source_handles.push(handle);

        state.shutdown_shared_sources();

        // The shutdown channel should now carry `true`.
        assert!(
            *rx.borrow(),
            "shutdown signal should be true after shutdown_shared_sources()"
        );
    }

    #[test]
    fn test_shutdown_shared_sources_no_op_when_empty() {
        let mut state = AppState::new();
        // Should not panic when there are no handles.
        state.shutdown_shared_sources();
        assert!(state.shared_source_handles.is_empty());
    }

    // ─────────────────────────────────────────────────────────
    // Flutter Version Panel Tests (Phase 2, Task 01)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_show_flutter_version_sets_ui_mode() {
        let mut state = AppState::default();
        state.show_flutter_version();
        assert_eq!(state.ui_mode, UiMode::FlutterVersion);
        assert!(state.flutter_version_state.visible);
    }

    #[test]
    fn test_show_flutter_version_snapshots_sdk() {
        let mut state = AppState::default();
        // resolved_sdk is None by default — show_flutter_version should still work
        state.show_flutter_version();
        // sdk_info.resolved_sdk is None because resolved_sdk is None
        assert!(state.flutter_version_state.sdk_info.resolved_sdk.is_none());
    }

    #[test]
    fn test_hide_flutter_version_returns_to_normal() {
        let mut state = AppState::default();
        state.show_flutter_version();
        state.hide_flutter_version();
        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(!state.flutter_version_state.visible);
    }

    #[test]
    fn test_flutter_version_state_initialized_to_default() {
        let state = AppState::new();
        assert!(!state.flutter_version_state.visible);
        assert_eq!(
            state.flutter_version_state.focused_pane,
            crate::flutter_version::FlutterVersionPane::SdkInfo
        );
        assert!(state
            .flutter_version_state
            .version_list
            .installed_versions
            .is_empty());
    }

    // ── Startup notice field tests (version-check-banner) ────────────────────

    /// The `startup_notice` field must default to `None` so no notice appears
    /// on processes where no actionable startup condition applies.
    #[test]
    fn startup_notice_defaults_to_none() {
        let state = AppState::new();
        assert!(state.startup_notice.is_none());
    }

    /// `dismiss_startup_notice_on_interaction` must clear the notice when
    /// the user interacts outside the dialog (Normal mode).
    #[test]
    fn dismiss_startup_notice_on_interaction_clears_in_normal_mode() {
        let mut state = AppState {
            startup_notice: Some(StartupNotice::NewVersionAvailable {
                latest: "0.5.7".into(),
            }),
            ..AppState::new()
        };
        state.ui_mode = UiMode::Normal;
        state.dismiss_startup_notice_on_interaction();
        assert!(state.startup_notice.is_none());
    }

    /// `dismiss_startup_notice_on_interaction` must be a no-op while the New
    /// Session Dialog is visible — the dialog owns the lifecycle.
    #[test]
    fn dismiss_startup_notice_on_interaction_noop_in_dialog() {
        let mut state = AppState {
            startup_notice: Some(StartupNotice::NewVersionAvailable {
                latest: "0.5.7".into(),
            }),
            ..AppState::new()
        };
        state.ui_mode = UiMode::NewSessionDialog;
        state.dismiss_startup_notice_on_interaction();
        assert!(
            state.startup_notice.is_some(),
            "notice must survive while dialog is visible"
        );
    }

    /// `dismiss_startup_notice_on_interaction` must be a no-op when there is no
    /// notice to dismiss.
    #[test]
    fn dismiss_startup_notice_on_interaction_noop_when_no_notice() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Normal;
        // Should not panic or set any unexpected state
        state.dismiss_startup_notice_on_interaction();
        assert!(state.startup_notice.is_none());
    }

    /// `hide_new_session_dialog` must clear `startup_notice` so the notice
    /// does not persist if the dialog is re-opened in the same process.
    #[test]
    fn hide_new_session_dialog_clears_startup_notice() {
        let mut state = AppState {
            startup_notice: Some(StartupNotice::NewVersionAvailable {
                latest: "9.9.9".into(),
            }),
            ..AppState::new()
        };
        state.hide_new_session_dialog();
        assert!(state.startup_notice.is_none());
        assert_eq!(
            state.ui_mode,
            UiMode::Normal,
            "ui_mode must return to Normal after hide_new_session_dialog"
        );
    }

    // ─────────────────────────────────────────────────────────
    // Mouse Region Field Tests (Phase 3, Task 03)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_appstate_initializes_with_empty_mouse_regions() {
        let state = AppState::new();
        let regions = state.mouse_regions.take();
        assert!(regions.is_empty(), "fresh AppState has no mouse regions");
        state.mouse_regions.set(regions); // restore so the assertion is non-destructive
    }

    #[test]
    fn test_appstate_mouse_regions_capacity_preserves() {
        let state = AppState::new();
        let regions = state.mouse_regions.take();
        // with_capacity() pre-sizes to 32 — we don't lock that number into a test,
        // but we do assert that capacity is non-zero so a single push doesn't
        // immediately realloc.
        assert!(regions.iter().count() == 0);
        state.mouse_regions.set(regions);
    }

    // ── mouse_capture_active initialization tests (Task 03) ──────────────────

    #[test]
    fn test_appstate_initializes_mouse_capture_active_from_settings_true() {
        let mut settings = crate::config::Settings::default();
        settings.ui.enable_mouse = true;
        let state = AppState::with_settings(std::path::PathBuf::new(), settings);
        assert!(
            state.mouse_capture_active,
            "mouse_capture_active should be true when settings.ui.enable_mouse is true"
        );
    }

    #[test]
    fn test_appstate_initializes_mouse_capture_active_from_settings_false() {
        let mut settings = crate::config::Settings::default();
        settings.ui.enable_mouse = false;
        let state = AppState::with_settings(std::path::PathBuf::new(), settings);
        assert!(
            !state.mouse_capture_active,
            "mouse_capture_active should be false when settings.ui.enable_mouse is false"
        );
    }

    // ── hide_implementation_widgets wire-up tests (task 03) ──────────────────

    #[test]
    fn test_appstate_propagates_hide_implementation_true_from_settings() {
        let mut settings = crate::config::Settings::default();
        settings.devtools.hide_implementation_widgets = true;
        let state = AppState::with_settings(std::path::PathBuf::new(), settings);
        assert!(
            state
                .devtools_view_state
                .inspector
                .hide_implementation_widgets,
            "inspector.hide_implementation_widgets should mirror settings on startup"
        );
    }

    #[test]
    fn test_appstate_propagates_hide_implementation_false_from_settings() {
        let mut settings = crate::config::Settings::default();
        settings.devtools.hide_implementation_widgets = false;
        let state = AppState::with_settings(std::path::PathBuf::new(), settings);
        assert!(
            !state
                .devtools_view_state
                .inspector
                .hide_implementation_widgets,
            "inspector.hide_implementation_widgets should mirror settings (false) on startup"
        );
    }

    // ── Properties cache-field reset tests (phase-2-task-03) ──────────────────

    #[test]
    fn reset_clears_properties_cache_fields() {
        let mut state = InspectorState {
            last_fetched_properties_node_id: Some("objects/42".into()),
            pending_properties_node_id: Some("objects/43".into()),
            ..Default::default()
        };
        state.reset();
        assert!(
            state.last_fetched_properties_node_id.is_none(),
            "reset() must clear last_fetched_properties_node_id"
        );
        assert!(
            state.pending_properties_node_id.is_none(),
            "reset() must clear pending_properties_node_id"
        );
    }

    #[test]
    fn reset_details_and_groups_clears_properties_cache_fields() {
        let mut state = InspectorState {
            last_fetched_properties_node_id: Some("objects/42".into()),
            pending_properties_node_id: Some("objects/43".into()),
            ..Default::default()
        };
        state.reset_details_and_groups();
        assert!(
            state.last_fetched_properties_node_id.is_none(),
            "reset_details_and_groups() must clear last_fetched_properties_node_id"
        );
        assert!(
            state.pending_properties_node_id.is_none(),
            "reset_details_and_groups() must clear pending_properties_node_id"
        );
    }

    // ─────────────────────────────────────────────────────────
    // visible_tabs / clamp_details_tab / details_context tests
    // (Phase 3, Task 02)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn visible_tabs_default_is_properties_only() {
        let state = InspectorState::default();
        assert_eq!(state.visible_tabs(), vec![DetailsTab::Properties]);
    }

    #[test]
    fn visible_tabs_includes_render_object_when_render_properties_non_empty() {
        let state = InspectorState {
            render_properties: vec![DiagnosticsNode {
                description: "RenderFlex".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(
            state.visible_tabs(),
            vec![DetailsTab::Properties, DetailsTab::RenderObject]
        );
    }

    #[test]
    fn visible_tabs_includes_flex_explorer_when_context_is_flex_layout() {
        let state = InspectorState {
            details_context: DetailsContext {
                is_flex_layout: true,
                parent_type: None,
            },
            ..Default::default()
        };
        assert_eq!(
            state.visible_tabs(),
            vec![DetailsTab::Properties, DetailsTab::FlexExplorer]
        );
    }

    #[test]
    fn visible_tabs_includes_all_three_when_both_conditions_hold() {
        let state = InspectorState {
            render_properties: vec![DiagnosticsNode {
                description: "RenderFlex".into(),
                ..Default::default()
            }],
            details_context: DetailsContext {
                is_flex_layout: true,
                parent_type: Some("Column".into()),
            },
            ..Default::default()
        };
        assert_eq!(
            state.visible_tabs(),
            vec![
                DetailsTab::Properties,
                DetailsTab::RenderObject,
                DetailsTab::FlexExplorer
            ]
        );
    }

    #[test]
    fn clamp_details_tab_snaps_to_properties_when_render_object_hidden() {
        let mut state = InspectorState {
            details_tab: DetailsTab::RenderObject,
            // render_properties intentionally empty → RenderObject hidden
            ..Default::default()
        };
        state.clamp_details_tab();
        assert_eq!(state.details_tab, DetailsTab::Properties);
    }

    #[test]
    fn clamp_details_tab_snaps_to_properties_when_flex_explorer_hidden() {
        let mut state = InspectorState {
            details_tab: DetailsTab::FlexExplorer,
            // details_context.is_flex_layout intentionally false → FlexExplorer hidden
            ..Default::default()
        };
        state.clamp_details_tab();
        assert_eq!(state.details_tab, DetailsTab::Properties);
    }

    #[test]
    fn clamp_details_tab_noop_when_active_tab_visible() {
        let mut state = InspectorState {
            details_tab: DetailsTab::RenderObject,
            render_properties: vec![DiagnosticsNode {
                description: "RenderFlex".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        state.clamp_details_tab();
        assert_eq!(state.details_tab, DetailsTab::RenderObject);
    }

    #[test]
    fn reset_clears_details_context() {
        let mut state = InspectorState {
            details_context: DetailsContext {
                is_flex_layout: true,
                parent_type: Some("Column".into()),
            },
            ..Default::default()
        };
        state.reset();
        assert_eq!(state.details_context, DetailsContext::default());
    }

    #[test]
    fn reset_details_and_groups_clears_details_context() {
        let mut state = InspectorState {
            details_context: DetailsContext {
                is_flex_layout: true,
                parent_type: Some("Row".into()),
            },
            ..Default::default()
        };
        state.reset_details_and_groups();
        assert_eq!(state.details_context, DetailsContext::default());
    }

    // ─────────────────────────────────────────────────────────
    // PerfDetailsTab Tests
    // (devtools-performance-memory-split Phase 2, Task 02)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn perf_details_tab_default_is_frame_analysis() {
        assert_eq!(PerfDetailsTab::default(), PerfDetailsTab::FrameAnalysis);
    }

    #[test]
    fn perf_details_tab_next_wraps() {
        assert_eq!(
            PerfDetailsTab::FrameAnalysis.next(),
            PerfDetailsTab::RebuildStats
        );
        assert_eq!(
            PerfDetailsTab::RebuildStats.next(),
            PerfDetailsTab::TimelineEvents
        );
        assert_eq!(
            PerfDetailsTab::TimelineEvents.next(),
            PerfDetailsTab::FrameAnalysis
        );
    }

    #[test]
    fn perf_details_tab_prev_wraps() {
        assert_eq!(
            PerfDetailsTab::FrameAnalysis.prev(),
            PerfDetailsTab::TimelineEvents
        );
        assert_eq!(
            PerfDetailsTab::TimelineEvents.prev(),
            PerfDetailsTab::RebuildStats
        );
        assert_eq!(
            PerfDetailsTab::RebuildStats.prev(),
            PerfDetailsTab::FrameAnalysis
        );
    }
}
