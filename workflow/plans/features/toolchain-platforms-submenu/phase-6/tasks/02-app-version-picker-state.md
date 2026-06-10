## Task: App — `VersionPickerState` module (grouping, arch filter, navigation)

**Objective**: Create the picker's data model as a new `install_wizard/version_picker.rs` module in
`fdemon-app`: `VersionPickerState` (visibility, fetch lifecycle, channel tabs, cursor/scroll,
confirmed selection), pure manifest grouping + arch filtering + synthetic master/main rows, and
navigation/clamp methods. Wire the field into `InstallWizardState`. No handler/message/TUI wiring —
that's Tasks 03/05.

**Depends on**: Task 01 (merged — `FlutterRelease.release_date` exists).

**Agent:** implementor

**Complexity:** medium

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/version_picker.rs` — **NEW**
- `crates/fdemon-app/src/install_wizard/state.rs` — add `pub version_picker: VersionPickerState`
  to `InstallWizardState` (+ Default wiring); drop manifest data on wizard hide (see Notes).
- `crates/fdemon-app/src/install_wizard/mod.rs` — `mod version_picker; pub use version_picker::*;`

**Files Read (Dependencies):**
- `fdemon_daemon::toolchain` — `FlutterRelease`, `FlutterReleaseManifest`, `HostArch`
  (all already `pub`; re-export from the daemon's `toolchain/mod.rs` is in place — verify, and if
  `HostArch` is somehow not re-exported, add it there rather than reaching into `types`).
- `crates/fdemon-app/src/flutter_version/state.rs` — `VersionListState` (`selected_index` +
  `scroll_offset` + `loading` + `error` + `Cell<usize>` render-hint) as the structural template.

### Details

> Locate by symbol; line numbers drift.

#### 1. Types

```rust
/// Channel tabs, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PickerChannel {
    #[default]
    Stable,
    Beta,
    Master, // synthetic git-only rows: "master", "main"
}

/// One selectable row.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerRow {
    pub version: String,               // "3.24.0" or "master"/"main"
    pub channel: String,               // release channel, or "master" for synthetic rows
    pub release_date: Option<String>,  // raw ISO-8601, None for synthetic/old rows
    pub arch: Option<String>,          // dart_sdk_arch passthrough
    pub git_only: bool,                // true only for the synthetic Master rows
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PickerFetch {
    #[default]
    NotFetched,
    Loading,
    Loaded,
    Failed, // error text in `error`
}

#[derive(Debug, Default)]
pub struct VersionPickerState {
    pub visible: bool,
    pub fetch: PickerFetch,
    pub error: Option<String>,
    pub tab: PickerChannel,
    /// Grouped rows per tab, built once per manifest fetch.
    pub stable: Vec<PickerRow>,
    pub beta: Vec<PickerRow>,
    pub master: Vec<PickerRow>,        // always exactly ["master", "main"]
    pub selected_index: usize,         // within the active tab
    pub scroll_offset: usize,
    pub last_known_visible_height: std::cell::Cell<usize>,
    /// Confirmed choice; survives picker close so Enter re-runs with it.
    pub selected_release: Option<PickerRow>,
}
```

(Derive `Clone` on the state only if `InstallWizardState` requires it — check; `Cell` is `Clone`-able.)

#### 2. Pure grouping/filter functions (unit-test these hard)

```rust
/// Group manifest releases into per-tab rows. Filters by host arch (exact
/// `dart_sdk_arch` match OR arch absent — older entries), preserves manifest
/// order (newest first), dedupes identical (version, channel) pairs left over
/// after arch filtering, and appends the synthetic git-only master rows.
pub fn group_releases(manifest: &FlutterReleaseManifest, arch: HostArch) -> (Vec<PickerRow>, Vec<PickerRow>, Vec<PickerRow>)
```

- Arch filter: keep a release when `dart_sdk_arch.is_none()` or it equals
  `arch.as_manifest_str()`. (`HostArch::Unknown` → keep everything.) Linux/Windows manifests are
  x64-only so the filter is a no-op there; macOS has dual-arch duplicates that must collapse to one
  row per version.
- `dev`-channel entries (deprecated) are dropped.
- Master tab rows: `master` then `main`, `git_only: true`, `channel: "master"`.
- Default cursor: index 0 of Stable (newest stable = active stable; `resolve_stable()` precedent —
  no hash plumbing).

#### 3. State methods (mirror the wizard/state idioms)

- `open(&mut self)` — `visible = true`; reset `tab` to Stable, `selected_index`/`scroll_offset` to 0
  **only when nothing is loaded yet** (re-opening keeps position); returns whether a fetch is needed
  (`fetch == NotFetched || Failed`).
- `close(&mut self)` — `visible = false` (keep manifest + selection).
- `begin_fetch(&mut self)` / `apply_manifest(&mut self, manifest, arch)` (groups, sets `Loaded`,
  clamps cursor) / `apply_fetch_error(&mut self, msg)`.
- `rows(&self) -> &[PickerRow]` for the active tab; `selected_row(&self) -> Option<&PickerRow>`.
- `move_up/move_down` — clamp to `rows().len()`; adjust `scroll_offset` using
  `last_known_visible_height` (the `VersionListState` keystroke-clamp pattern).
- `next_tab(&mut self)` — Stable → Beta → Master → Stable; reset `selected_index`/`scroll_offset`.
- `confirm(&mut self) -> Option<PickerRow>` — clone the selected row into `selected_release`,
  `close()`, return it. `None` when the active tab is empty.
- `clear_manifest(&mut self)` — back to `NotFetched`, drop the three vecs (memory release on wizard
  hide); **keep `selected_release`** only if the wizard stays open — on full wizard hide clear it
  too (single method `reset()` is fine).

#### 4. `InstallWizardState` wiring (`state.rs`)

- Add the field + `Default`.
- In the wizard's hide/escape reset path (locate the existing `visible = false` handling — likely
  `handle_hide`/`escape` in `handler/install_wizard/navigation.rs` calls a state method; **only the
  state-side method changes here**): call `version_picker.reset()`. If the reset currently lives
  purely in the handler (Task 03's file), expose `reset()` here and leave the call site to Task 03 —
  coordinate via the method, don't edit navigation.rs in this task.

### Acceptance Criteria

1. `group_releases` on a fixture manifest: stable/beta split correct, newest-first order preserved,
   `dev` dropped, macOS dual-arch collapsed to host arch, arch-less old entries kept, master tab is
   exactly `["master", "main"]` with `git_only: true`.
2. Navigation: cursor clamps at both ends per tab; tab switch resets cursor + scroll; scroll follows
   the cursor given a small `last_known_visible_height`.
3. `confirm` stores + returns the row and hides the picker; empty-tab confirm is a no-op `None`.
4. `open` after `Failed` reports fetch-needed again; `apply_manifest` after `r`-refetch regroups and
   clamps a now-out-of-range cursor.
5. `reset` drops rows + selection and returns to `NotFetched`.
6. `cargo test -p fdemon-app --lib install_wizard` green; fmt + clippy clean.

### Testing

```bash
cargo test -p fdemon-app --lib install_wizard::version_picker
cargo test -p fdemon-app --lib
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

Build a small local manifest fixture (a `FlutterReleaseManifest` literal — no JSON parsing needed at
this layer) with: 2 stable (one dual-arch duplicated), 1 beta, 1 dev, 1 arch-less stable.

### Notes

- **No messages, no handlers, no TUI** in this task — keep it a pure data module so Tasks 03 and 05
  can build against it in parallel.
- Module placement follows the Modular Design guideline: `state.rs` is already >4k lines; the picker
  gets its own file with its own test mod.
- `PickerRow` (not `FlutterRelease`) crosses into the TUI so the widget doesn't depend on daemon
  types beyond what `fdemon-app` re-exposes — match how `flutter_version/types.rs` re-exports
  `InstalledSdk` if a re-export is preferred.
- Keep ordering deterministic everywhere — the TUI tests and `[`-style cycling depend on it.
