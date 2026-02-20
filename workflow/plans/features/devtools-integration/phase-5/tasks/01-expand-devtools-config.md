## Task: Expand DevTools Configuration

**Objective**: Expand the `DevToolsSettings` struct with all planned configuration fields from PLAN.md and wire each field into the corresponding behavior in handlers and actions.

**Depends on**: None

**Estimated Time**: 4-6 hours

### Scope

- `crates/fdemon-app/src/config/types.rs`: Expand `DevToolsSettings` struct with new fields
- `crates/fdemon-app/src/handler/devtools.rs`: Use config values for default panel, overlay defaults
- `crates/fdemon-app/src/actions.rs`: Use config values for performance refresh interval, memory history size, tree max depth
- `crates/fdemon-app/src/session/performance.rs`: Configurable ring buffer sizes
- `crates/fdemon-core/src/performance.rs`: Accept configurable sizes for `RingBuffer` constructors
- `crates/fdemon-app/src/config/settings_items.rs`: Add new settings to the settings panel TUI (if applicable)

### Details

#### 1. Expand `DevToolsSettings` Struct

Current state at `config/types.rs:277-287`:

```rust
pub struct DevToolsSettings {
    pub auto_open: bool,
    pub browser: String,
}
```

Target state (from PLAN.md lines 622-659):

```rust
/// DevTools settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DevToolsSettings {
    /// Auto-open DevTools when app starts
    #[serde(default)]
    pub auto_open: bool,

    /// Browser to use (empty = system default)
    #[serde(default)]
    pub browser: String,

    /// Default panel when entering DevTools mode
    #[serde(default = "default_devtools_panel")]
    pub default_panel: String,  // "inspector", "layout", "performance"

    /// Performance data refresh interval in milliseconds
    #[serde(default = "default_performance_refresh_ms")]
    pub performance_refresh_ms: u64,

    /// Memory history size (number of snapshots to retain)
    #[serde(default = "default_memory_history_size")]
    pub memory_history_size: usize,

    /// Widget tree max fetch depth (0 = unlimited)
    #[serde(default)]
    pub tree_max_depth: u32,

    /// Auto-enable repaint rainbow on VM connect
    #[serde(default)]
    pub auto_repaint_rainbow: bool,

    /// Auto-enable performance overlay on VM connect
    #[serde(default)]
    pub auto_performance_overlay: bool,

    /// Logging sub-settings
    #[serde(default)]
    pub logging: DevToolsLoggingSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DevToolsLoggingSettings {
    /// Enable hybrid logging (VM Service + daemon)
    #[serde(default = "default_true")]
    pub hybrid_enabled: bool,

    /// Prefer VM Service log level when available
    #[serde(default = "default_true")]
    pub prefer_vm_level: bool,

    /// Show log source indicator ([VM] vs [daemon])
    #[serde(default)]
    pub show_source_indicator: bool,

    /// Dedupe threshold: logs within N ms with same message are duplicates
    #[serde(default = "default_dedupe_threshold_ms")]
    pub dedupe_threshold_ms: u64,
}
```

Provide default value functions:
- `default_devtools_panel()` → `"inspector".to_string()`
- `default_performance_refresh_ms()` → `2000` (current hardcoded value in actions.rs)
- `default_memory_history_size()` → `60` (current hardcoded value in performance.rs)
- `default_dedupe_threshold_ms()` → `100`

#### 2. Wire `default_panel` into Enter DevTools Handler

In `handler/devtools.rs`, `handle_enter_devtools_mode()` currently defaults to `DevToolsPanel::Inspector`. Change to read `state.settings.devtools.default_panel` and map `"layout"` → `DevToolsPanel::Layout`, `"performance"` → `DevToolsPanel::Performance`, else `DevToolsPanel::Inspector`.

#### 3. Wire `performance_refresh_ms` into Performance Polling

In `actions.rs`, the performance polling interval is currently hardcoded (likely `Duration::from_secs(2)`). Replace with `Duration::from_millis(settings.devtools.performance_refresh_ms)`. The settings value needs to be passed through the `UpdateAction::StartPerformanceMonitoring` variant or captured when spawning the polling task.

#### 4. Wire `memory_history_size` into Ring Buffer Sizes

In `session/performance.rs`, `PerformanceState::new()` likely creates ring buffers with hardcoded sizes (60 for memory, 300 for frames, 50 for GC). Make the memory history size configurable via the settings. Consider whether frame and GC history sizes should also be configurable (keep simple — only memory for now unless trivial to add).

#### 5. Wire `tree_max_depth` into Widget Tree Fetch

In `actions.rs`, the `FetchWidgetTree` action calls `getRootWidgetTree` or `getRootWidgetSummaryTree`. Pass `tree_max_depth` as a parameter to the RPC call if the Flutter extension supports a depth limit. If not supported by the extension, this field can limit the depth during client-side tree parsing.

#### 6. Wire `auto_*_overlay` into VM Connect Handler

When a VM Service connects (`handle_vm_service_connected` or similar), if `auto_repaint_rainbow` or `auto_performance_overlay` is true, automatically dispatch the corresponding `ToggleOverlay` action. Be careful not to fire these if the overlay is already in the desired state — always read-then-set (the existing toggle pattern reads first).

#### 7. Wire Logging Settings

Evaluate which logging settings are already hard-coded behaviors from Phase 1:
- `hybrid_enabled`: Check if there's a flag controlling hybrid vs daemon-only logging
- `prefer_vm_level`: Check if VM log level takes precedence in log merging
- `show_source_indicator`: Check if `[VM]`/`[daemon]` tags exist in log rendering
- `dedupe_threshold_ms`: Check if dedup logic exists and what threshold it uses

For each, replace the hard-coded value with the config field. If the behavior doesn't exist yet, add it.

#### 8. Add to Settings Panel (Optional)

If `settings_items.rs` has a devtools section, add the new fields. The settings panel already shows `auto_open` and `browser` — extend with the new fields using the existing `SettingsItem` pattern.

### Acceptance Criteria

1. `DevToolsSettings` struct has all planned fields with `serde(default)` for backwards compatibility
2. `DevToolsLoggingSettings` sub-struct exists with all planned fields
3. Existing configs with only `auto_open` and `browser` continue to deserialize without error
4. `default_panel` controls which panel opens when entering DevTools mode
5. `performance_refresh_ms` controls the polling interval for memory snapshots
6. `memory_history_size` controls the ring buffer capacity for memory history
7. `tree_max_depth` is passed to widget tree fetch (if supported) or tree trimming
8. `auto_repaint_rainbow` / `auto_performance_overlay` auto-enable overlays on VM connect
9. Logging settings wire into existing hybrid logging behavior
10. All new fields appear in settings panel (if applicable)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_devtools_settings_default_values() {
        let settings = DevToolsSettings::default();
        assert!(!settings.auto_open);
        assert!(settings.browser.is_empty());
        assert_eq!(settings.default_panel, "inspector");
        assert_eq!(settings.performance_refresh_ms, 2000);
        assert_eq!(settings.memory_history_size, 60);
        assert_eq!(settings.tree_max_depth, 0);
        assert!(!settings.auto_repaint_rainbow);
        assert!(!settings.auto_performance_overlay);
    }

    #[test]
    fn test_devtools_settings_backwards_compatible_deserialization() {
        // Old config with only auto_open and browser should still work
        let toml = r#"
            auto_open = true
            browser = "firefox"
        "#;
        let settings: DevToolsSettings = toml::from_str(toml).unwrap();
        assert!(settings.auto_open);
        assert_eq!(settings.browser, "firefox");
        // New fields should have defaults
        assert_eq!(settings.default_panel, "inspector");
        assert_eq!(settings.performance_refresh_ms, 2000);
    }

    #[test]
    fn test_devtools_settings_full_deserialization() {
        let toml = r#"
            auto_open = false
            browser = ""
            default_panel = "performance"
            performance_refresh_ms = 5000
            memory_history_size = 120
            tree_max_depth = 10
            auto_repaint_rainbow = true
            auto_performance_overlay = false

            [logging]
            hybrid_enabled = true
            prefer_vm_level = false
            show_source_indicator = true
            dedupe_threshold_ms = 200
        "#;
        let settings: DevToolsSettings = toml::from_str(toml).unwrap();
        assert_eq!(settings.default_panel, "performance");
        assert_eq!(settings.performance_refresh_ms, 5000);
        assert_eq!(settings.memory_history_size, 120);
        assert_eq!(settings.tree_max_depth, 10);
        assert!(settings.auto_repaint_rainbow);
        assert!(settings.logging.show_source_indicator);
        assert_eq!(settings.logging.dedupe_threshold_ms, 200);
    }

    #[test]
    fn test_default_panel_maps_to_devtools_panel_enum() {
        // Test the mapping logic (wherever it lives)
        assert_eq!(parse_default_panel("inspector"), DevToolsPanel::Inspector);
        assert_eq!(parse_default_panel("layout"), DevToolsPanel::Layout);
        assert_eq!(parse_default_panel("performance"), DevToolsPanel::Performance);
        assert_eq!(parse_default_panel("invalid"), DevToolsPanel::Inspector); // fallback
    }
}
```

### Notes

- **Backwards compatibility is critical.** All new fields must have `serde(default)` so existing `.fdemon/config.toml` files continue to work.
- **The `[devtools.logging]` section** maps to a nested TOML table. In the Rust struct, use `#[serde(default)]` on the `logging: DevToolsLoggingSettings` field so the entire sub-table is optional.
- **Performance refresh interval minimum**: Consider clamping `performance_refresh_ms` to a minimum of 500ms to prevent excessive polling.
- **Ring buffer sizes**: When `memory_history_size` changes at runtime (via settings panel), the existing ring buffer doesn't need to resize — the new size takes effect on the next session start.
