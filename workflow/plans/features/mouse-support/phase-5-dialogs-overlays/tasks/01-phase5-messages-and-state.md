## Task: Phase-5 Messages, AppState Fields & Dispatch Stubs

**Objective**: Define every new `Message` variant Phase 5 will emit, add the `AppState::last_settings_click` field that powers Settings double-click detection, and wire dispatch arms in `handler/update.rs` and `handler/settings_handlers.rs` so the rest of the phase can compile against stable APIs. Stub function bodies are added so the build stays green; Tasks 03 and 04 fill in the bodies.

**Depends on**: None (Wave 1)

**Estimated Time**: 1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs`: Add five new `Message` variants in a new `// ── Mouse Click Messages (Phase 5) ──` section near the existing Phase-4 mouse-click section. Document each with a `///` rustdoc comment.
- `crates/fdemon-app/src/state.rs`: Add `pub last_settings_click: Option<SettingsClickStamp>` to `AppState`. Define `pub struct SettingsClickStamp { pub index: usize, pub at: std::time::Instant }` (alongside `LogClickStamp`). Initialise to `None` in `AppState::new()` / `with_settings()`.
- `crates/fdemon-app/src/handler/update.rs`: Add five `Message::*` match arms. Three (`NewSessionDialogSelectDeviceAt`, `NewSessionDialogFocusField`, `NewSessionDialogFuzzySelectAt`) delegate to (yet-to-be-filled) handlers in `handler/new_session/`. One (`SettingsClickRow`) delegates to `handler/settings_handlers::handle_settings_click_row`. One (`TagFilterClickRow`) is implemented inline as a stub returning `UpdateResult::none()` (Task 04 fills in the body).
- `crates/fdemon-app/src/handler/settings_handlers.rs`: Add stub `pub fn handle_settings_click_row(state: &mut AppState, index: usize) -> UpdateResult { UpdateResult::none() }`. Body is Task 03's responsibility.
- `crates/fdemon-app/src/handler/new_session/navigation.rs` (or a new `clicks.rs`): Add stubs `pub fn handle_select_device_at(state: &mut AppState, index: usize) -> UpdateResult`, `pub fn handle_focus_field(state: &mut AppState, field: LaunchContextField) -> UpdateResult`, `pub fn handle_fuzzy_select_at(state: &mut AppState, index: usize) -> UpdateResult`. All return `UpdateResult::none()` for now; Tasks 03/04/09 will fill them in (or merge them into existing handlers — the per-task author can choose).

  *Implementation hint*: the cleanest factoring is a NEW `handler/new_session/clicks.rs` submodule that holds all three click handlers; the existing `navigation.rs` file remains focused on relative navigation. The mod.rs of `handler/new_session/` declares `mod clicks;` and the dispatch arms in `update.rs` call `crate::handler::new_session::clicks::handle_select_device_at(...)` etc. This task creates the file with stub bodies; Phase 5 has no dedicated task to fill these in (they are small enough to be filled in by Task 09 alongside the widget regions, or Task 03/04 if those tasks want the full chain working before widget regions land).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs::LogClickStamp` (Phase 4 precedent for the `SettingsClickStamp` struct shape).
- `crates/fdemon-app/src/new_session_dialog/launch_context.rs` (for `LaunchContextField` enum).
- `crates/fdemon-app/src/handler/log_view.rs::handle_click_log_row` (Phase 4 chained-message double-click template — Task 03 will mirror this).

### Details

#### New `Message` variants

Add to the `Message` enum in `message.rs`. Place them in a new `// ── Mouse Click Messages (Phase 5) ──` section immediately after the Phase-4 section:

```rust
// ── Mouse Click Messages (Phase 5) ────────────────────────────────────────

/// Click on a device row inside the NewSessionDialog Connected/Bootable list.
///
/// `index` is the absolute position into the *currently active* tab's device
/// list at render time (Connected or Bootable, whichever was visible). The
/// handler at [`crate::handler::new_session::clicks::handle_select_device_at`]
/// sets `target_selector.selected_index = index` for the active tab and emits
/// a follow-up [`Message::NewSessionDialogDeviceSelect`] via
/// [`UpdateResult::message`] so the click is exactly equivalent to "arrow
/// down N times then Enter".
NewSessionDialogSelectDeviceAt { index: usize },

/// Click on a launch-context field row (Configuration / Mode / Flavor /
/// Entry Point / Dart Defines).
///
/// Sets `launch_context.focused_field = field` and emits a follow-up
/// [`Message::NewSessionDialogFieldActivate`] via [`UpdateResult::message`]
/// for fields that activate-on-Enter. The Mode field's left/right cycler is
/// not exercised by click in v1 — clicking the Mode field activates the
/// existing keyboard-Enter behaviour (cycle to next mode).
NewSessionDialogFocusField {
    field: crate::new_session_dialog::LaunchContextField,
},

/// Click on a result row inside the NewSessionDialog fuzzy modal
/// (config picker, flavor picker, entry-point picker).
///
/// Sets `fuzzy_modal.selected_index = index` and emits a follow-up
/// [`Message::NewSessionDialogFuzzyConfirm`] via [`UpdateResult::message`].
/// Equivalent to "arrow down N times then Enter" inside the modal.
NewSessionDialogFuzzySelectAt { index: usize },

/// Click on a setting row in the Settings panel.
///
/// `index` is the absolute position into the active tab's `SettingItem` list
/// at render time. The handler at
/// [`crate::handler::settings_handlers::handle_settings_click_row`] updates
/// `AppState::last_settings_click` for double-click detection and sets
/// `settings_view_state.selected_index = index`. When the same row is
/// clicked twice within 400 ms, a follow-up
/// [`Message::SettingsToggleEdit`] is emitted via
/// [`UpdateResult::message`] (mirroring [`Message::ClickLogRow`]).
SettingsClickRow { index: usize },

/// Click on a tag row in the tag-filter overlay.
///
/// `index` is the absolute position into the *sorted* tag list at render
/// time. The inline handler in `update.rs` sets
/// `tag_filter_ui.selected_index = index` AND toggles the tag's visibility
/// in a single arm — no follow-up message. Single click both navigates to
/// and toggles the tag, since there is no useful "select-without-toggle"
/// state in this overlay (the user wants both).
TagFilterClickRow { index: usize },
```

#### `AppState::last_settings_click`

In `state.rs`, immediately after the existing `LogClickStamp` definition:

```rust
/// Click stamp recorded by [`handler::settings_handlers::handle_settings_click_row`]
/// to detect double-clicks on a setting row within the 400 ms window.
///
/// Mirrors [`LogClickStamp`] — see Phase 4 task 01 for the precedent.
#[derive(Debug, Clone, Copy)]
pub struct SettingsClickStamp {
    pub index: usize,
    pub at: std::time::Instant,
}

// In `AppState`:
pub struct AppState {
    // ...
    /// Most recent settings-row click, used for double-click detection.
    /// Cleared whenever a double-click is consumed or the active tab
    /// changes.
    pub last_settings_click: Option<SettingsClickStamp>,
}

// In `AppState::new()` / `AppState::with_settings()`:
last_settings_click: None,
```

#### Dispatch arms in `handler/update.rs`

Add five new arms in the existing `match msg { ... }`. Place them adjacent to the existing Phase-4 mouse-click arms (the section that contains `Message::ClickLogRow`):

```rust
// ── Mouse Click Messages (Phase 5) ────────────────────────────────────

Message::NewSessionDialogSelectDeviceAt { index } => {
    crate::handler::new_session::clicks::handle_select_device_at(state, index)
}

Message::NewSessionDialogFocusField { field } => {
    crate::handler::new_session::clicks::handle_focus_field(state, field)
}

Message::NewSessionDialogFuzzySelectAt { index } => {
    crate::handler::new_session::clicks::handle_fuzzy_select_at(state, index)
}

Message::SettingsClickRow { index } => {
    crate::handler::settings_handlers::handle_settings_click_row(state, index)
}

Message::TagFilterClickRow { index: _ } => {
    // Stub. Body added in Phase 5 Task 04.
    UpdateResult::none()
}
```

#### Stub function signatures

In `handler/settings_handlers.rs` (append before any existing tests module, matching the file's organisation):

```rust
/// Stub. Body added in Phase 5 Task 03.
pub fn handle_settings_click_row(_state: &mut AppState, _index: usize) -> UpdateResult {
    UpdateResult::none()
}
```

Create a NEW `crates/fdemon-app/src/handler/new_session/clicks.rs`:

```rust
//! Click handlers for the NewSessionDialog (Phase 5).
//!
//! These functions are dispatched from `handler/update.rs` when a click
//! produces an absolute-index `Message` (see `Message::NewSessionDialog*At`,
//! `Message::NewSessionDialogFocusField`). They mutate state and emit
//! follow-up messages to chain into the existing relative-navigation flow.

use crate::message::Message;
use crate::new_session_dialog::LaunchContextField;
use crate::state::AppState;

use super::super::UpdateResult;

/// Stub. Body added in Phase 5 Task 09.
pub fn handle_select_device_at(_state: &mut AppState, _index: usize) -> UpdateResult {
    // TODO(Phase 5 Task 09): set target_selector.selected_index = index for the active tab,
    // emit Message::NewSessionDialogDeviceSelect as a follow-up.
    let _ = Message::NewSessionDialogDeviceSelect; // keep variant alive-ref for clippy
    UpdateResult::none()
}

/// Stub. Body added in Phase 5 Task 09.
pub fn handle_focus_field(_state: &mut AppState, _field: LaunchContextField) -> UpdateResult {
    // TODO(Phase 5 Task 09): set launch_context.focused_field = field,
    // emit Message::NewSessionDialogFieldActivate as a follow-up.
    let _ = Message::NewSessionDialogFieldActivate;
    UpdateResult::none()
}

/// Stub. Body added in Phase 5 Task 09.
pub fn handle_fuzzy_select_at(_state: &mut AppState, _index: usize) -> UpdateResult {
    // TODO(Phase 5 Task 09): set fuzzy_modal.selected_index = index,
    // emit Message::NewSessionDialogFuzzyConfirm as a follow-up.
    let _ = Message::NewSessionDialogFuzzyConfirm;
    UpdateResult::none()
}
```

Add `mod clicks;` to `crates/fdemon-app/src/handler/new_session/mod.rs` to expose the submodule (the `pub fn`s become callable as `crate::handler::new_session::clicks::*`).

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes after this task — every new variant has a dispatch arm; every dispatch arm has a stub function.
2. `cargo test --workspace` passes (no behavioural changes — stubs return `UpdateResult::none()`; new fields default to `None`).
3. `cargo fmt --all -- --check` passes.
4. `cargo clippy --workspace --all-targets -- -D warnings` passes — all unused `_state` / `_index` etc. are explicitly underscore-prefixed.
5. `Message::NewSessionDialogSelectDeviceAt`, `Message::NewSessionDialogFocusField`, `Message::NewSessionDialogFuzzySelectAt`, `Message::SettingsClickRow`, `Message::TagFilterClickRow` exist with the field shapes specified above.
6. `AppState::new()` and `AppState::with_settings()` both set `last_settings_click: None`.
7. `SettingsClickStamp { index: usize, at: std::time::Instant }` is defined in `state.rs` and is `Copy + Clone + Debug`.
8. Each stub function carries a `/// Stub. Body added in Phase 5 Task NN.` doc-comment so reviewers don't mistake it for production logic.
9. The new `handler/new_session/clicks.rs` file exists and is reachable from `update.rs` via `crate::handler::new_session::clicks::*`.

### Testing

Add no production tests in this task — every stub returns `None`. Existing tests must continue passing.

If a `tests.rs` exists in any touched module, ensure no test references the new variant in a way that would force a stub to do work.

### Notes

- **Why a `Copy` `SettingsClickStamp`.** Same as `LogClickStamp` — `Instant: Copy`, `usize: Copy`. The `Copy` derive simplifies the read-then-clear pattern in Task 03 (`let last = state.last_settings_click; state.last_settings_click = None;`).
- **Why `TagFilterClickRow`'s arm body is inline (vs `handler/settings_handlers.rs`-style delegation).** `TagFilterMoveUp`, `TagFilterMoveDown`, `TagFilterToggleSelected` all have inline arm bodies in `update.rs` — the tag-filter handlers don't have a dedicated module. `TagFilterClickRow` follows the same convention.
- **Why a NEW `handler/new_session/clicks.rs` file vs appending to `navigation.rs`.** `navigation.rs` is focused on *relative* navigation (Up/Down/FieldNext/FieldPrev). Click handlers operate on *absolute* positions and return chained messages. A separate file makes the distinction discoverable and prevents `navigation.rs` from growing into a grab-bag.
- **Why stubs don't `todo!()`.** `todo!()` panics during integration tests in Task 11 if the dispatch arms get hit before Tasks 03 / 04 / 09 land. Returning `UpdateResult::none()` is harmless — it just means the click is silently ignored until the body is filled in.
- **No `Eq` derivation needed for new variants.** `Message` already does not derive `Eq`. The new variants use only `usize` and an `enum`-typed `LaunchContextField` — they are trivially `PartialEq` if the parent enum is. Don't add `Eq` derives.
- **`LaunchContextField` is already in `crate::new_session_dialog`** (re-exported from the app layer). Confirm with a `Cargo check` that the import resolves; if it doesn't, the module's `pub use` declaration may need a one-line addition.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added 5 new `Message` variants in a new `// ── Mouse Click Messages (Phase 5) ──` section: `NewSessionDialogSelectDeviceAt`, `NewSessionDialogFocusField`, `NewSessionDialogFuzzySelectAt`, `SettingsClickRow`, `TagFilterClickRow` |
| `crates/fdemon-app/src/state.rs` | Added `SettingsClickStamp` struct (Copy+Clone+Debug) immediately after `LogClickStamp`; added `last_settings_click: Option<SettingsClickStamp>` field to `AppState`; initialized to `None` in `with_settings()` |
| `crates/fdemon-app/src/handler/update.rs` | Added 5 new dispatch arms in Phase 5 section adjacent to Phase 4 mouse-click arms |
| `crates/fdemon-app/src/handler/settings_handlers.rs` | Added stub `pub fn handle_settings_click_row` before the `#[cfg(test)]` module |
| `crates/fdemon-app/src/handler/new_session/mod.rs` | Added `pub mod clicks;` declaration |
| `crates/fdemon-app/src/handler/new_session/clicks.rs` | New file with 3 stub click handlers: `handle_select_device_at`, `handle_focus_field`, `handle_fuzzy_select_at` |

### Notable Decisions/Tradeoffs

1. **`pub mod clicks` vs `mod clicks`**: Used `pub mod clicks` so `update.rs` can call `crate::handler::new_session::clicks::*` — the module path must be publicly reachable through the crate root.
2. **Blank line removal in fmt**: `cargo fmt` removed blank lines after `// ── Mouse Click Messages (Phase 5) ──` section headers and the single-brace import. Applied fixes before committing.

### Testing Performed

- `cargo check --workspace --all-targets` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)
- `cargo fmt --all -- --check` - Passed
- `cargo test --workspace` - Passed (all test suites pass, no regressions)

### Risks/Limitations

1. **Stub bodies**: All three `new_session/clicks.rs` functions and `handle_settings_click_row` return `UpdateResult::none()`. They are intentional stubs for Tasks 03, 04, and 09 to fill in.
