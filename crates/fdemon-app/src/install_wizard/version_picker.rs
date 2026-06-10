//! # Version Picker State
//!
//! Data model for the Flutter version picker overlay in the Install Wizard.
//! The picker lets users select a specific Flutter version (or a git-only
//! `master`/`main` ref) before installation.
//!
//! This module is **pure data**: no messages, handlers, or TUI wiring.
//! Task 03 (handler) and Task 05 (TUI widget) build on top of it.
//!
//! ## Design Notes
//!
//! - `PickerRow` is the unit of display; it is derived from `FlutterRelease`
//!   but does not carry daemon-internal fields (archive URL, sha256).  This
//!   means the TUI only needs `fdemon-app`, not `fdemon-daemon` directly.
//! - `group_releases` is a pure function — deterministic, no I/O.  The only
//!   impure operation is `apply_manifest`, which calls it and mutates state.

use std::cell::Cell;

use fdemon_daemon::toolchain::{FlutterReleaseManifest, HostArch};

// ── Channel Tabs ──────────────────────────────────────────────────────────────

/// The three channel tabs shown in the version picker, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PickerChannel {
    /// The stable Flutter release channel (default tab).
    #[default]
    Stable,
    /// The beta Flutter release channel.
    Beta,
    /// Synthetic git-only rows: `"master"` and `"main"`.
    Master,
}

// ── Row types ─────────────────────────────────────────────────────────────────

/// One selectable row in the version picker list.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerRow {
    /// Display version string: `"3.24.0"` for real releases, `"master"` /
    /// `"main"` for the synthetic git-only rows.
    pub version: String,
    /// Release channel label: `"stable"`, `"beta"`, or `"master"` for synthetic
    /// rows.
    pub channel: String,
    /// Raw ISO-8601 release date from the manifest (e.g.
    /// `"2024-08-21T17:10:03.737Z"`), or `None` for synthetic / old entries
    /// that lack the field.
    pub release_date: Option<String>,
    /// The `dart_sdk_arch` string from the manifest entry that produced this
    /// row, or `None` for entries that predate the multi-arch field.
    pub arch: Option<String>,
    /// `true` only for the two synthetic Master tab rows (`"master"`, `"main"`).
    /// These rows trigger a `git clone` rather than an archive download.
    pub git_only: bool,
}

// ── Fetch lifecycle ───────────────────────────────────────────────────────────

/// Lifecycle state of the manifest fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PickerFetch {
    /// No fetch has been attempted yet (initial state).
    #[default]
    NotFetched,
    /// A fetch is in-flight.
    Loading,
    /// Manifest was fetched and rows are populated.
    Loaded,
    /// Fetch failed; error text is in [`VersionPickerState::error`].
    Failed,
}

// ── Main state ────────────────────────────────────────────────────────────────

/// State for the Flutter version picker overlay.
///
/// Owned by [`super::InstallWizardState`] as `version_picker`.
/// Kept behind a sub-field so the rest of the wizard state is unaffected.
///
/// Lifecycle:
/// 1. Created via `Default` (invisible, nothing fetched).
/// 2. `open()` → returns whether a fetch is needed.
/// 3. `begin_fetch()` / `apply_manifest()` or `apply_fetch_error()`.
/// 4. User navigates with `move_up` / `move_down` / `next_tab`.
/// 5. `confirm()` → returns `Some(PickerRow)` and hides the picker.
/// 6. On full wizard hide: `reset()`.
pub struct VersionPickerState {
    /// Whether the picker overlay is visible.
    pub visible: bool,
    /// Current fetch lifecycle state.
    pub fetch: PickerFetch,
    /// Error message from a failed fetch (`fetch == Failed`).
    pub error: Option<String>,
    /// Which channel tab is active.
    pub tab: PickerChannel,
    /// Grouped rows for the Stable tab, newest-first.
    pub stable: Vec<PickerRow>,
    /// Grouped rows for the Beta tab, newest-first.
    pub beta: Vec<PickerRow>,
    /// Synthetic master rows: always exactly `["master", "main"]`.
    pub master: Vec<PickerRow>,
    /// Currently selected row index within the active tab.
    pub selected_index: usize,
    /// Scroll offset for the active tab list.
    pub scroll_offset: usize,
    /// Render-hint: actual visible height from the last rendered frame.
    ///
    /// Follows the `Cell<usize>` render-hint pattern
    /// (see docs/CODE_STANDARDS.md Principle 3).
    /// Defaults to 0, which signals "not yet rendered — use fallback".
    /// Written by the renderer; never mutated by message handlers.
    pub last_known_visible_height: Cell<usize>,
    /// The confirmed selection. Survives picker close so that pressing Enter
    /// on the install step re-uses the last chosen version without re-opening
    /// the picker.
    pub selected_release: Option<PickerRow>,
}

impl Default for VersionPickerState {
    fn default() -> Self {
        Self {
            visible: false,
            fetch: PickerFetch::NotFetched,
            error: None,
            tab: PickerChannel::Stable,
            stable: Vec::new(),
            beta: Vec::new(),
            master: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            last_known_visible_height: Cell::new(0),
            selected_release: None,
        }
    }
}

impl std::fmt::Debug for VersionPickerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VersionPickerState")
            .field("visible", &self.visible)
            .field("fetch", &self.fetch)
            .field("error", &self.error)
            .field("tab", &self.tab)
            .field("stable_count", &self.stable.len())
            .field("beta_count", &self.beta.len())
            .field("master_count", &self.master.len())
            .field("selected_index", &self.selected_index)
            .field("scroll_offset", &self.scroll_offset)
            .field(
                "last_known_visible_height",
                &self.last_known_visible_height.get(),
            )
            .field("selected_release", &self.selected_release)
            .finish()
    }
}

impl VersionPickerState {
    // ── Lifecycle ─────────────────────────────────────────────────────────────

    /// Open the picker.
    ///
    /// Sets `visible = true`. Resets `tab`, `selected_index`, and
    /// `scroll_offset` **only when nothing is loaded yet** (i.e. the fetch is
    /// `NotFetched` or `Failed`). Re-opening after a successful load keeps the
    /// user's position.
    ///
    /// Returns `true` when a manifest fetch is needed (`NotFetched` or
    /// `Failed`), `false` when the manifest is already loaded or loading.
    pub fn open(&mut self) -> bool {
        self.visible = true;
        let needs_fetch = matches!(self.fetch, PickerFetch::NotFetched | PickerFetch::Failed);
        if needs_fetch {
            // Reset navigation to a clean starting position.
            self.tab = PickerChannel::Stable;
            self.selected_index = 0;
            self.scroll_offset = 0;
        }
        needs_fetch
    }

    /// Close the picker without confirming a selection.
    ///
    /// Hides the overlay but keeps the manifest rows and the last
    /// `selected_release` intact so re-opening is cheap.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Transition to `Loading` state in preparation for a manifest fetch.
    ///
    /// Called by the handler just before dispatching the async fetch task.
    pub fn begin_fetch(&mut self) {
        self.fetch = PickerFetch::Loading;
        self.error = None;
    }

    /// Apply a fetched manifest, group its releases, and update fetch state.
    ///
    /// Groups releases by channel (arch-filtered), sets `fetch = Loaded`, and
    /// clamps `selected_index` in case a re-fetch produced fewer rows.
    pub fn apply_manifest(&mut self, manifest: &FlutterReleaseManifest, arch: HostArch) {
        let (stable, beta, master) = group_releases(manifest, arch);
        self.stable = stable;
        self.beta = beta;
        self.master = master;
        self.fetch = PickerFetch::Loaded;
        self.error = None;
        // Clamp cursor in case the new list is shorter.
        self.clamp_cursor();
    }

    /// Record a fetch failure and transition to `Failed` state.
    pub fn apply_fetch_error(&mut self, msg: impl Into<String>) {
        self.fetch = PickerFetch::Failed;
        self.error = Some(msg.into());
    }

    // ── Row access ────────────────────────────────────────────────────────────

    /// Return the rows for the currently active tab.
    pub fn rows(&self) -> &[PickerRow] {
        match self.tab {
            PickerChannel::Stable => &self.stable,
            PickerChannel::Beta => &self.beta,
            PickerChannel::Master => &self.master,
        }
    }

    /// Return the currently selected row, or `None` when the active tab is
    /// empty or the cursor is out of bounds.
    pub fn selected_row(&self) -> Option<&PickerRow> {
        self.rows().get(self.selected_index)
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    /// Move the cursor up by one row, clamping at index 0.
    ///
    /// Adjusts `scroll_offset` using the `last_known_visible_height` render-hint
    /// (see docs/CODE_STANDARDS.md Principle 3).
    pub fn move_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
        self.adjust_scroll();
    }

    /// Move the cursor down by one row, clamping at the last row.
    ///
    /// Adjusts `scroll_offset` using the `last_known_visible_height` render-hint.
    pub fn move_down(&mut self) {
        let len = self.rows().len();
        if len > 0 && self.selected_index < len - 1 {
            self.selected_index += 1;
        }
        self.adjust_scroll();
    }

    /// Cycle to the next tab in order: Stable → Beta → Master → Stable.
    ///
    /// Resets `selected_index` and `scroll_offset` when the tab changes.
    pub fn next_tab(&mut self) {
        self.tab = match self.tab {
            PickerChannel::Stable => PickerChannel::Beta,
            PickerChannel::Beta => PickerChannel::Master,
            PickerChannel::Master => PickerChannel::Stable,
        };
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    // ── Confirm / Reset ───────────────────────────────────────────────────────

    /// Confirm the current selection.
    ///
    /// Clones the selected row into `selected_release`, calls `close()`, and
    /// returns the row. Returns `None` when the active tab is empty (no-op).
    pub fn confirm(&mut self) -> Option<PickerRow> {
        let row = self.selected_row()?.clone();
        self.selected_release = Some(row.clone());
        self.close();
        Some(row)
    }

    /// Drop all manifest rows and return to `NotFetched`.
    ///
    /// Called during a normal manifest memory-release (wizard hide while wizard
    /// stays open). Keeps `selected_release` only when the wizard is still open
    /// — call `reset()` (which also clears `selected_release`) on full wizard
    /// close.
    pub fn clear_manifest(&mut self) {
        self.stable.clear();
        self.beta.clear();
        self.master.clear();
        self.fetch = PickerFetch::NotFetched;
        self.error = None;
    }

    /// Full reset: drop all state including the confirmed selection.
    ///
    /// Called when the wizard is fully closed (hidden). Returns the picker
    /// to a pristine `default()` state so the next `open()` starts fresh.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Clamp `selected_index` to the active tab's row count.
    fn clamp_cursor(&mut self) {
        let len = self.rows().len();
        if len == 0 {
            self.selected_index = 0;
            self.scroll_offset = 0;
        } else if self.selected_index >= len {
            self.selected_index = len - 1;
            self.adjust_scroll();
        }
    }

    /// Adjust `scroll_offset` so the selected row remains visible.
    ///
    /// Uses `last_known_visible_height` as the viewport height (Principle 3).
    /// Falls back to a small default when the render-hint has not been set yet.
    fn adjust_scroll(&mut self) {
        /// Fallback height used before the first render frame sets the hint.
        const DEFAULT_VISIBLE_HEIGHT: usize = 10;

        let visible_height = match self.last_known_visible_height.get() {
            0 => DEFAULT_VISIBLE_HEIGHT,
            h => h,
        };

        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected_index - visible_height + 1;
        }
    }
}

// ── Pure grouping function ─────────────────────────────────────────────────────

/// Group manifest releases into per-tab `(stable, beta, master)` row vectors.
///
/// ## Rules
///
/// - **Arch filter**: keep a release when `dart_sdk_arch.is_none()` **or** it
///   equals `arch.as_manifest_str()`. `HostArch::Unknown` → keep everything.
/// - **`dev` channel**: entries with `channel == "dev"` are always dropped.
/// - **Deduplication**: after arch-filtering, identical `(version, channel)` pairs
///   are deduplicated (left-fold, first occurrence wins) so macOS dual-arch
///   duplicates collapse to a single row per version.
/// - **Order**: manifest order (newest-first) is preserved throughout.
/// - **Master tab**: two synthetic git-only rows, `"master"` then `"main"`,
///   with `channel: "master"` and `git_only: true`.  These are not derived from
///   manifest entries.
///
/// Returns `(stable_rows, beta_rows, master_rows)`.
pub fn group_releases(
    manifest: &FlutterReleaseManifest,
    arch: HostArch,
) -> (Vec<PickerRow>, Vec<PickerRow>, Vec<PickerRow>) {
    let arch_label = arch.as_manifest_str();

    let mut stable: Vec<PickerRow> = Vec::new();
    let mut beta: Vec<PickerRow> = Vec::new();

    for release in &manifest.releases {
        // Drop deprecated dev-channel entries.
        if release.channel == "dev" {
            continue;
        }

        // Arch filter: keep when arch is absent (pre-multi-arch entry) or matches.
        let arch_ok = match (&release.dart_sdk_arch, arch_label) {
            // Entry has no arch field → always keep (old manifest entry).
            (None, _) => true,
            // We don't know the host arch → keep everything.
            (Some(_), None) => true,
            // Both present → must match.
            (Some(entry_arch), Some(host_arch)) => entry_arch == host_arch,
        };
        if !arch_ok {
            continue;
        }

        let row = PickerRow {
            version: release.version.clone(),
            channel: release.channel.clone(),
            release_date: release.release_date.clone(),
            arch: release.dart_sdk_arch.clone(),
            git_only: false,
        };

        let target = if release.channel == "stable" {
            &mut stable
        } else {
            &mut beta
        };

        // Deduplicate: skip if (version, channel) already present.
        let already_present = target
            .iter()
            .any(|r| r.version == row.version && r.channel == row.channel);
        if !already_present {
            target.push(row);
        }
    }

    // Synthetic master tab: always exactly these two rows.
    let master = vec![
        PickerRow {
            version: "master".to_string(),
            channel: "master".to_string(),
            release_date: None,
            arch: None,
            git_only: true,
        },
        PickerRow {
            version: "main".to_string(),
            channel: "master".to_string(),
            release_date: None,
            arch: None,
            git_only: true,
        },
    ];

    (stable, beta, master)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_daemon::toolchain::{FlutterRelease, FlutterReleaseManifest, HostArch};

    // ── Fixture helpers ───────────────────────────────────────────────────────

    fn make_release(
        version: &str,
        channel: &str,
        arch: Option<&str>,
        date: Option<&str>,
    ) -> FlutterRelease {
        FlutterRelease {
            version: version.to_string(),
            channel: channel.to_string(),
            archive: format!("{channel}/{version}.tar.xz"),
            sha256: "abc123".to_string(),
            dart_sdk_arch: arch.map(str::to_string),
            release_date: date.map(str::to_string),
        }
    }

    /// Minimal manifest with a controlled set of releases for unit testing.
    ///
    /// Layout (newest first):
    /// - 3.24.0 stable  arm64  (dual-arch pair — macOS scenario)
    /// - 3.24.0 stable  x64
    /// - 3.22.0 stable  x64
    /// - 3.10.0 stable  None   (old, no arch field)
    /// - 2.0.0  beta    x64
    /// - 1.5.0  dev     x64    (deprecated — must be dropped)
    fn make_fixture_manifest() -> FlutterReleaseManifest {
        FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![
                make_release("3.24.0", "stable", Some("arm64"), Some("2024-08-21")),
                make_release("3.24.0", "stable", Some("x64"), Some("2024-08-21")),
                make_release("3.22.0", "stable", Some("x64"), Some("2024-06-01")),
                make_release("3.10.0", "stable", None, None),
                make_release("2.0.0", "beta", Some("x64"), Some("2024-01-15")),
                make_release("1.5.0", "dev", Some("x64"), None),
            ],
        }
    }

    // ── group_releases tests ──────────────────────────────────────────────────

    #[test]
    fn test_group_releases_stable_beta_split_correct() {
        let manifest = make_fixture_manifest();
        let (stable, beta, _master) = group_releases(&manifest, HostArch::X64);
        // Stable rows: 3.24.0, 3.22.0, 3.10.0 (arch-less) — dev dropped
        assert_eq!(stable.len(), 3, "expected 3 stable rows, got {stable:?}");
        // Beta rows: 2.0.0 only
        assert_eq!(beta.len(), 1, "expected 1 beta row, got {beta:?}");
    }

    #[test]
    fn test_group_releases_dev_dropped() {
        let manifest = make_fixture_manifest();
        let (stable, beta, _) = group_releases(&manifest, HostArch::X64);
        let all_channels: Vec<_> = stable
            .iter()
            .chain(beta.iter())
            .map(|r| r.channel.as_str())
            .collect();
        assert!(
            !all_channels.contains(&"dev"),
            "dev channel should be dropped: {all_channels:?}"
        );
    }

    #[test]
    fn test_group_releases_newest_first_order_preserved() {
        let manifest = make_fixture_manifest();
        let (stable, _beta, _) = group_releases(&manifest, HostArch::X64);
        // Manifest order is preserved: 3.24.0, 3.22.0, 3.10.0
        let versions: Vec<_> = stable.iter().map(|r| r.version.as_str()).collect();
        assert_eq!(versions, vec!["3.24.0", "3.22.0", "3.10.0"]);
    }

    #[test]
    fn test_group_releases_macos_dual_arch_collapsed_to_host_arch() {
        let manifest = make_fixture_manifest();
        // Filter for arm64 — the arm64 row for 3.24.0 is kept, x64 is dropped.
        let (stable, _, _) = group_releases(&manifest, HostArch::Arm64);
        let v324: Vec<_> = stable.iter().filter(|r| r.version == "3.24.0").collect();
        assert_eq!(
            v324.len(),
            1,
            "dual-arch 3.24.0 should collapse to 1 row, got {v324:?}"
        );
        assert_eq!(v324[0].arch.as_deref(), Some("arm64"));
    }

    #[test]
    fn test_group_releases_macos_x64_gets_x64_row() {
        let manifest = make_fixture_manifest();
        let (stable, _, _) = group_releases(&manifest, HostArch::X64);
        let v324: Vec<_> = stable.iter().filter(|r| r.version == "3.24.0").collect();
        assert_eq!(v324.len(), 1, "dual-arch 3.24.0 should collapse to 1 row");
        assert_eq!(v324[0].arch.as_deref(), Some("x64"));
    }

    #[test]
    fn test_group_releases_arch_less_old_entries_kept() {
        let manifest = make_fixture_manifest();
        // 3.10.0 has no arch field — must be kept regardless of host arch.
        for arch in [HostArch::X64, HostArch::Arm64, HostArch::Unknown] {
            let (stable, _, _) = group_releases(&manifest, arch);
            let found = stable.iter().any(|r| r.version == "3.10.0");
            assert!(found, "arch-less 3.10.0 should be kept for arch={arch:?}");
        }
    }

    #[test]
    fn test_group_releases_master_tab_is_master_and_main() {
        let manifest = make_fixture_manifest();
        let (_, _, master) = group_releases(&manifest, HostArch::X64);
        assert_eq!(master.len(), 2);
        assert_eq!(master[0].version, "master");
        assert_eq!(master[1].version, "main");
        assert!(master[0].git_only);
        assert!(master[1].git_only);
        assert_eq!(master[0].channel, "master");
        assert_eq!(master[1].channel, "master");
    }

    #[test]
    fn test_group_releases_unknown_arch_keeps_all() {
        let manifest = make_fixture_manifest();
        let (stable, beta, _) = group_releases(&manifest, HostArch::Unknown);
        // Unknown arch → keep all (both arm64 and x64 for 3.24.0).
        // After dedup, 3.24.0 arm64 comes first so x64 is dropped. Still 1 row.
        // Plus 3.22.0 (x64), 3.10.0 (none) → 3 stable rows.
        assert_eq!(
            stable.len(),
            3,
            "Unknown arch: expected 3 deduplicated stable rows, got {stable:?}"
        );
        assert_eq!(beta.len(), 1);
    }

    #[test]
    fn test_group_releases_git_only_false_for_real_releases() {
        let manifest = make_fixture_manifest();
        let (stable, beta, _) = group_releases(&manifest, HostArch::X64);
        for row in stable.iter().chain(beta.iter()) {
            assert!(
                !row.git_only,
                "real release should have git_only=false: {row:?}"
            );
        }
    }

    // ── Navigation tests ──────────────────────────────────────────────────────

    fn picker_with_rows(count: usize) -> VersionPickerState {
        let mut state = VersionPickerState::default();
        for i in 0..count {
            state.stable.push(PickerRow {
                version: format!("3.{i}.0"),
                channel: "stable".to_string(),
                release_date: None,
                arch: None,
                git_only: false,
            });
        }
        state.fetch = PickerFetch::Loaded;
        state
    }

    #[test]
    fn test_move_up_clamps_at_zero() {
        let mut state = picker_with_rows(3);
        state.selected_index = 0;
        state.move_up();
        assert_eq!(state.selected_index, 0, "cursor should clamp at 0");
    }

    #[test]
    fn test_move_down_clamps_at_last() {
        let mut state = picker_with_rows(3);
        state.selected_index = 2;
        state.move_down();
        assert_eq!(state.selected_index, 2, "cursor should clamp at last row");
    }

    #[test]
    fn test_move_down_advances_cursor() {
        let mut state = picker_with_rows(3);
        state.selected_index = 0;
        state.move_down();
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_move_up_retreats_cursor() {
        let mut state = picker_with_rows(3);
        state.selected_index = 2;
        state.move_up();
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_tab_switch_resets_cursor_and_scroll() {
        let mut state = picker_with_rows(5);
        state.selected_index = 3;
        state.scroll_offset = 2;
        state.next_tab(); // Stable → Beta
        assert_eq!(state.selected_index, 0, "tab switch should reset cursor");
        assert_eq!(state.scroll_offset, 0, "tab switch should reset scroll");
    }

    #[test]
    fn test_next_tab_cycles_stable_beta_master_stable() {
        let mut state = VersionPickerState::default();
        assert_eq!(state.tab, PickerChannel::Stable);
        state.next_tab();
        assert_eq!(state.tab, PickerChannel::Beta);
        state.next_tab();
        assert_eq!(state.tab, PickerChannel::Master);
        state.next_tab();
        assert_eq!(state.tab, PickerChannel::Stable);
    }

    #[test]
    fn test_scroll_follows_cursor_with_small_visible_height() {
        let mut state = picker_with_rows(20);
        // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md
        state.last_known_visible_height.set(5);
        // Move the cursor far down; scroll_offset should follow.
        for _ in 0..15 {
            state.move_down();
        }
        assert_eq!(state.selected_index, 15);
        // scroll_offset should be at most selected_index - visible_height + 1 = 11
        assert!(
            state.scroll_offset <= state.selected_index,
            "scroll must not exceed cursor"
        );
        assert!(
            state.selected_index < state.scroll_offset + 5,
            "cursor must be within visible window"
        );
    }

    // ── Confirm tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_confirm_stores_and_returns_row_and_hides_picker() {
        let mut state = picker_with_rows(3);
        state.visible = true;
        state.selected_index = 1;
        let result = state.confirm();
        assert!(result.is_some(), "confirm should return a row");
        let row = result.unwrap();
        assert_eq!(row.version, "3.1.0");
        assert!(state.selected_release.is_some());
        assert_eq!(state.selected_release.unwrap().version, "3.1.0");
        assert!(!state.visible, "confirm should hide the picker");
    }

    #[test]
    fn test_confirm_empty_tab_returns_none() {
        let mut state = VersionPickerState {
            visible: true,
            fetch: PickerFetch::Loaded,
            ..VersionPickerState::default()
        };
        // No rows in any tab → confirm is a no-op.
        let result = state.confirm();
        assert!(result.is_none(), "empty tab confirm should return None");
        assert!(
            state.visible,
            "picker should remain visible on empty confirm"
        );
    }

    // ── open / fetch lifecycle tests ──────────────────────────────────────────

    #[test]
    fn test_open_after_failed_reports_fetch_needed() {
        let mut state = VersionPickerState {
            fetch: PickerFetch::Failed,
            error: Some("network error".to_string()),
            ..VersionPickerState::default()
        };
        let needs_fetch = state.open();
        assert!(needs_fetch, "open after Failed should report fetch needed");
        assert!(state.visible);
        // Navigation reset after Failed
        assert_eq!(state.tab, PickerChannel::Stable);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_open_after_loaded_does_not_reset_position() {
        let mut state = picker_with_rows(5);
        state.visible = true;
        state.fetch = PickerFetch::Loaded;
        state.selected_index = 3;
        state.scroll_offset = 1;
        // Close without reset.
        state.close();
        assert!(!state.visible);
        // Re-open: position should be preserved.
        let needs_fetch = state.open();
        assert!(!needs_fetch, "open after Loaded should not require a fetch");
        assert!(state.visible);
        assert_eq!(state.selected_index, 3, "position should survive re-open");
        assert_eq!(state.scroll_offset, 1);
    }

    #[test]
    fn test_apply_manifest_regroups_and_clamps_out_of_range_cursor() {
        // Put the cursor at index 5 (out of range for the new manifest).
        let mut state = VersionPickerState {
            selected_index: 5,
            fetch: PickerFetch::Loaded,
            stable: (0..10)
                .map(|i| PickerRow {
                    version: format!("3.{i}.0"),
                    channel: "stable".to_string(),
                    release_date: None,
                    arch: None,
                    git_only: false,
                })
                .collect(),
            ..VersionPickerState::default()
        };

        // Now apply a small manifest with only 3 stable releases.
        let small_manifest = FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![
                make_release("3.2.0", "stable", Some("x64"), None),
                make_release("3.1.0", "stable", Some("x64"), None),
                make_release("3.0.0", "stable", Some("x64"), None),
            ],
        };
        state.apply_manifest(&small_manifest, HostArch::X64);

        assert_eq!(state.stable.len(), 3);
        assert!(
            state.selected_index < 3,
            "cursor should be clamped: selected_index={}",
            state.selected_index
        );
        assert_eq!(state.fetch, PickerFetch::Loaded);
    }

    // ── reset / clear_manifest tests ─────────────────────────────────────────

    #[test]
    fn test_reset_drops_rows_and_selection_and_returns_to_not_fetched() {
        let mut state = picker_with_rows(5);
        state.fetch = PickerFetch::Loaded;
        state.selected_release = Some(PickerRow {
            version: "3.0.0".to_string(),
            channel: "stable".to_string(),
            release_date: None,
            arch: None,
            git_only: false,
        });
        state.visible = true;
        state.reset();
        assert_eq!(state.stable.len(), 0, "reset should clear stable rows");
        assert_eq!(state.beta.len(), 0, "reset should clear beta rows");
        assert_eq!(state.master.len(), 0, "reset should clear master rows");
        assert!(
            state.selected_release.is_none(),
            "reset should clear selected_release"
        );
        assert_eq!(state.fetch, PickerFetch::NotFetched);
        assert!(!state.visible);
    }

    #[test]
    fn test_clear_manifest_keeps_selected_release() {
        let mut state = picker_with_rows(5);
        state.fetch = PickerFetch::Loaded;
        state.selected_release = Some(PickerRow {
            version: "3.0.0".to_string(),
            channel: "stable".to_string(),
            release_date: None,
            arch: None,
            git_only: false,
        });
        state.clear_manifest();
        assert_eq!(state.stable.len(), 0);
        assert_eq!(state.fetch, PickerFetch::NotFetched);
        // selected_release is kept.
        assert!(
            state.selected_release.is_some(),
            "clear_manifest should keep selected_release"
        );
    }

    #[test]
    fn test_begin_fetch_sets_loading() {
        let mut state = VersionPickerState::default();
        state.begin_fetch();
        assert_eq!(state.fetch, PickerFetch::Loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_apply_fetch_error_sets_failed() {
        let mut state = VersionPickerState::default();
        state.apply_fetch_error("network timeout");
        assert_eq!(state.fetch, PickerFetch::Failed);
        assert_eq!(state.error.as_deref(), Some("network timeout"));
    }
}
