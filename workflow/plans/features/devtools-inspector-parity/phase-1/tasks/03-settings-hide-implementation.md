## Task: Add `hide_implementation_widgets` to `[devtools]` settings and wire it through to `InspectorState`

**Objective**: Make the new `InspectorState.hide_implementation_widgets` field configurable via `.fdemon/config.toml` and persisted across application restarts.

**Depends on**: 02-state-inspector-extensions

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/config/types.rs`: Add the new field to `DevToolsSettings`.
- `crates/fdemon-app/src/config/settings.rs`: Update the documented example config string + any explicit-loader code that lists the field.
- ONE additional engine/state init site (implementor to verify with grep — see Details). Most likely: `crates/fdemon-app/src/engine.rs` or wherever `Settings` is bridged into `AppState` on startup.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` (for the `InspectorState.hide_implementation_widgets` field added in task 02).

### Details

#### 1. Add the setting

In `crates/fdemon-app/src/config/types.rs:319+` (the `DevToolsSettings` struct definition):

```rust
pub struct DevToolsSettings {
    // …existing fields…

    /// Whether to collapse long single-child chains of non-local-project
    /// wrapper widgets in the Inspector tree. When `true` (default), DevTools'
    /// `_alwaysVisible` heuristic is applied — chains of implementation
    /// widgets fold behind a `+ N more widgets` leader row. Toggle at runtime
    /// with `Shift+H`.
    #[serde(default = "default_hide_implementation_widgets")]
    pub hide_implementation_widgets: bool,
}

fn default_hide_implementation_widgets() -> bool { true }
```

Update the existing `impl Default for DevToolsSettings` block to initialize the field with `default_hide_implementation_widgets()`. Verify by reading `types.rs:424+` for the existing Default impl shape.

#### 2. Documentation example

In `crates/fdemon-app/src/config/settings.rs:470` (and `:678` if separate) where the `[devtools]` section is documented in the example config string, add a commented entry:

```toml
[devtools]
# Hide implementation widgets in the inspector tree.
# When true, long chains of wrapper widgets (BlocProvider, etc.) collapse
# into a "+ N more widgets" leader row. Toggle at runtime with Shift+H.
hide_implementation_widgets = true
```

#### 3. Wire-up site

Find where `Settings` is loaded and applied to `AppState`. Likely candidates (implementor verifies with `grep -rn "Settings::load\|settings: Settings" crates/fdemon-app/src/`):

- `crates/fdemon-app/src/engine.rs::Engine::new`
- `crates/fdemon-app/src/state.rs::AppState::new` (if it takes Settings)
- `crates/fdemon-app/src/handler/...` for runtime-load paths

At the bridge site, copy `settings.devtools.hide_implementation_widgets` into `state.devtools_view_state.inspector.hide_implementation_widgets` once at startup. Example:

```rust
state.devtools_view_state.inspector.hide_implementation_widgets =
    settings.devtools.hide_implementation_widgets;
```

#### 4. Persistence on toggle (deferred to task 05, but design note here)

Task 05's `handle_toggle_hide_implementation` is responsible for both:
1. Flipping the flag in `state`.
2. Writing the new value back to `.fdemon/config.toml`.

If the codebase already has a `save_settings()` or similar helper (search: `grep -rn "fn save\|fn persist" crates/fdemon-app/src/config/`), task 05 reuses it.

If no write-back helper exists, **document the gap in this task's Completion Summary** so task 05 can choose between (a) implementing a minimal `Settings::write_to(&self, path)` helper here and reusing it in the handler, or (b) deferring persistence to a follow-up task (in-memory toggle only for Phase 1 ship).

#### 5. Tests

In `crates/fdemon-app/src/config/types.rs`'s existing test module:

- `test_devtools_settings_default_has_hide_implementation_true` — verifies default.
- `test_devtools_settings_deserializes_hide_implementation_false` — round-trip toml with `hide_implementation_widgets = false`.
- `test_devtools_settings_deserializes_omitted_field_uses_default` — confirm `#[serde(default)]` kicks in.

### Acceptance Criteria

1. `[devtools] hide_implementation_widgets = true` round-trips through `.fdemon/config.toml`.
2. Omitting the field from the TOML uses the default value of `true`.
3. On application startup, `state.devtools_view_state.inspector.hide_implementation_widgets` reflects the value loaded from settings.
4. `cargo test -p fdemon-app` passes with the new tests.
5. `cargo clippy -p fdemon-app --all-targets -- -D warnings` passes.

### Testing

```rust
#[test]
fn test_devtools_settings_default_has_hide_implementation_true() {
    let settings = DevToolsSettings::default();
    assert!(settings.hide_implementation_widgets);
}

#[test]
fn test_devtools_settings_deserializes_hide_implementation_false() {
    let toml_str = "[devtools]\nhide_implementation_widgets = false\n";
    let parsed: Settings = toml::from_str(toml_str).unwrap();
    assert!(!parsed.devtools.hide_implementation_widgets);
}
```

### Notes

- Do NOT modify `InspectorState::default()` to read from settings — `Default` impls must be settings-agnostic. The wire-up happens at the bridge site only.
- If the wire-up site turns out to be inside `state.rs` (e.g., a constructor that takes Settings), coordinate carefully — task 02 modifies that same file. Confirm before starting that the wire-up site is in a separate file (engine.rs or similar); if it's in state.rs, this task overlaps with task 02 and must be sequenced after 02 completes.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/types.rs` | Added `hide_implementation_widgets: bool` field to `DevToolsSettings` with `#[serde(default = "default_hide_implementation_widgets")]`; added `default_hide_implementation_widgets() -> bool { true }` function; updated `Default` impl to include the field; added 3 new tests |
| `crates/fdemon-app/src/config/settings.rs` | Updated both example `[devtools]` TOML config strings (at the init-file path and the `generate_config_header` path) to include the `hide_implementation_widgets` entry with comments |
| `crates/fdemon-app/src/state.rs` | Wired `settings.devtools.hide_implementation_widgets` into `devtools_view_state.inspector.hide_implementation_widgets` inside `AppState::with_settings`; added 2 wire-up verification tests |

### Notable Decisions/Tradeoffs

1. **Wire-up site is `AppState::with_settings`**: The task identifies this as the correct bridge site (not `InspectorState::default()`). The `Default` impl remains settings-agnostic. The explicit propagation in `with_settings` is guarded by a comment explaining the pattern.
2. **`save_settings` helper already exists**: `crates/fdemon-app/src/config/settings.rs:522` has `pub fn save_settings(project_path: &Path, settings: &Settings) -> Result<()>`. Task 05 can directly call this to persist the runtime toggle — no new write helper needed.
3. **Extra wire-up tests added**: Two extra `AppState::with_settings` tests were added to `state.rs` (alongside the existing mouse-capture pattern) to explicitly verify the bridge is working. This exceeded the minimal spec but provides robust regression coverage.

### Testing Performed

- `cargo check -p fdemon-app` — Passed
- `cargo test -p fdemon-app` — Passed (2306 tests: 2306 passed, 0 failed)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed
- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Persistence on toggle deferred to task 05**: `save_settings` exists and is ready — task 05 just needs to call it after flipping `state.settings.devtools.hide_implementation_widgets` in the toggle handler.
