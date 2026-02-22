## Task: Add Network Configuration Options

**Objective**: Add configurable network settings (`max_network_entries`, `network_auto_record`, `network_poll_interval_ms`) to `DevToolsSettings`, wire them into `NetworkState` initialization, and update the generated default `config.toml` template to document all `[devtools]` fields (including existing ones that are currently undocumented).

**Depends on**: None

### Scope

- `crates/fdemon-app/src/config/types.rs`: MODIFIED — Add 3 new fields to `DevToolsSettings`
- `crates/fdemon-app/src/config/settings.rs`: MODIFIED — Update `generate_default_config()` template
- `crates/fdemon-app/src/session/network.rs`: MODIFIED — Accept config values in constructor
- `crates/fdemon-app/src/session/session.rs`: MODIFIED — Pass config to `NetworkState` initialization
- `crates/fdemon-app/src/process.rs` or `actions.rs`: MODIFIED — Pass poll interval to network monitoring task (if applicable)

### Details

#### 1. Add fields to `DevToolsSettings` (`config/types.rs`)

Add 3 new fields after `allocation_profile_interval_ms`:

```rust
/// Maximum number of network entries to keep per session (FIFO eviction).
/// Default: 500.
#[serde(default = "default_max_network_entries")]
pub max_network_entries: usize,

/// Whether to auto-start network recording when entering the Network tab.
/// Default: true.
#[serde(default = "default_network_auto_record")]
pub network_auto_record: bool,

/// Network profile polling interval in milliseconds.
/// Controls how often `getHttpProfile` is called when recording.
/// Clamped to minimum 500ms. Default: 1000.
#[serde(default = "default_network_poll_interval_ms")]
pub network_poll_interval_ms: u64,
```

Add default functions:

```rust
fn default_max_network_entries() -> usize {
    500
}

fn default_network_auto_record() -> bool {
    true
}

fn default_network_poll_interval_ms() -> u64 {
    1000
}
```

Update the `Default` impl to include the new fields.

#### 2. Wire into `NetworkState` initialization

In `session/network.rs`, add a constructor that accepts config values:

```rust
impl NetworkState {
    /// Create a new `NetworkState` with configurable settings.
    pub fn with_config(max_entries: usize, auto_record: bool) -> Self {
        Self {
            max_entries,
            recording: auto_record,
            ..Self::default()
        }
    }
}
```

In `session/session.rs`, wherever `NetworkState::default()` is called during session creation, pass the settings through. The `Session::new()` method or wherever `NetworkState` is initialized should accept `&DevToolsSettings` or the individual values.

Check `process.rs` / `actions.rs` for where the network polling task is spawned — if the poll interval is currently hardcoded, replace it with the configured `network_poll_interval_ms`. Clamp to minimum 500ms.

#### 3. Update generated default config template

In `config/settings.rs`, update `generate_default_config()` to document **all** `[devtools]` fields — not just `auto_open` and `browser`. The current template is:

```toml
[devtools]
auto_open = false
browser = ""            # Empty = system default
```

Replace with:

```toml
[devtools]
auto_open = false
browser = ""                          # Empty = system default
default_panel = "inspector"           # "inspector", "performance", or "network"
performance_refresh_ms = 2000         # Memory polling interval (min 500ms)
memory_history_size = 60              # Memory snapshots to retain
tree_max_depth = 0                    # Widget tree depth (0 = unlimited)
allocation_profile_interval_ms = 5000 # Class allocation fetch interval (min 1000ms)
max_network_entries = 500             # Max HTTP entries per session (FIFO eviction)
network_auto_record = true            # Auto-start recording when entering Network tab
network_poll_interval_ms = 1000       # HTTP profile poll interval (min 500ms)
```

Note: Do NOT include `auto_repaint_rainbow`, `auto_performance_overlay`, or the `[devtools.logging]` sub-section in the generated template — these are advanced settings that most users won't need. Keep the template focused on commonly tuned values.

#### 4. Update existing test for generated config

The test `test_generate_default_config_is_valid_toml` in `settings.rs` already validates that the generated config parses as valid TOML. No changes needed to the test itself, but verify it still passes after the template update.

Consider adding a test that the new fields round-trip correctly:

```rust
#[test]
fn test_default_config_includes_network_settings() {
    let content = generate_default_config();
    assert!(content.contains("max_network_entries"));
    assert!(content.contains("network_auto_record"));
    assert!(content.contains("network_poll_interval_ms"));
}
```

### Acceptance Criteria

1. `DevToolsSettings` has `max_network_entries`, `network_auto_record`, `network_poll_interval_ms` fields with correct defaults
2. `NetworkState::with_config()` constructor exists and is used during session creation
3. `NetworkState.max_entries` is set from `settings.devtools.max_network_entries`
4. `NetworkState.recording` initial value is set from `settings.devtools.network_auto_record`
5. Network poll interval is configurable (not hardcoded)
6. Generated `config.toml` documents all `[devtools]` fields
7. Generated `config.toml` remains valid TOML (existing test passes)
8. Existing config files without the new fields still load correctly (serde defaults)
9. `cargo check -p fdemon-app` passes
10. `cargo test -p fdemon-app` passes

### Testing

```bash
cargo test -p fdemon-app -- devtools
cargo test -p fdemon-app -- config
cargo test -p fdemon-app -- network
cargo test -p fdemon-app -- generate_default_config
```

### Notes

- **Backwards compatibility**: The `#[serde(default = "...")]` annotations ensure existing `config.toml` files without the new fields continue to work.
- **Poll interval clamping**: The minimum 500ms clamp should happen at the polling task level (where the interval is used), not in the config type. This matches the existing pattern for `performance_refresh_ms`.
- **`reset()` preservation**: `NetworkState::reset()` already preserves `max_entries`. Verify it also preserves the `recording` default or if it should reset to the configured `auto_record` value.
