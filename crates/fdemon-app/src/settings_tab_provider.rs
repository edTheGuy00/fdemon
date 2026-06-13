//! Generic, content-free extension seam for host-injected settings tabs.
//!
//! The settings panel renders four built-in tabs (Project, User, Launch,
//! VSCode). A host application embedding the engine can inject additional tabs
//! by pushing [`SettingsTabProvider`] trait objects onto
//! [`crate::state::AppState::extra_settings_tabs`]. The panel renders, edits,
//! and persists those tabs through this trait without knowing anything about
//! their content — no field names, ids, or section labels are baked into the
//! public crate.
//!
//! The trait is intentionally minimal: a provider supplies a tab title, builds
//! its item list on demand, accepts committed/toggled items back to mutate its
//! in-memory model, and persists that model to disk.

use std::path::Path;

use crate::config::SettingItem;

/// A host-supplied settings tab.
///
/// Implementors own an in-memory model of their settings. The settings panel
/// drives them through the same edit/commit plumbing used by the built-in tabs:
///
/// 1. [`items`](Self::items) is called whenever the tab is rendered or its item
///    count is needed; it rebuilds the displayed rows from the model.
/// 2. When the user toggles/cycles/commits a row, the panel hands the resulting
///    [`SettingItem`] (id + new value) back via [`apply`](Self::apply) so the
///    provider can update its model without the panel knowing field names.
/// 3. On save, [`save`](Self::save) persists the model and validates input,
///    returning a human-readable error string on failure.
///
/// The [`std::fmt::Debug`] supertrait keeps `#[derive(Debug)]` on `AppState`
/// working with a `Vec<Box<dyn SettingsTabProvider>>` field. `Send + Sync` are
/// required because the owning `AppState` crosses the async boundary in the
/// engine runner.
pub trait SettingsTabProvider: std::fmt::Debug + Send + Sync {
    /// Tab-bar label, supplied at runtime.
    fn title(&self) -> &str;

    /// Build the list of setting rows from the provider's in-memory model.
    fn items(&self) -> Vec<SettingItem>;

    /// Apply a committed or toggled item back to the in-memory model.
    ///
    /// The panel passes the whole [`SettingItem`] (id + value) so the provider
    /// can route by id without exposing field names to the panel.
    fn apply(&mut self, item: &SettingItem);

    /// Persist the in-memory model to disk. Validation happens here; on failure
    /// return a human-readable error message.
    fn save(&self, project_path: &Path) -> Result<(), String>;

    /// Whether this tab is read-only (no editing). Defaults to `false`.
    fn is_readonly(&self) -> bool {
        false
    }
}
