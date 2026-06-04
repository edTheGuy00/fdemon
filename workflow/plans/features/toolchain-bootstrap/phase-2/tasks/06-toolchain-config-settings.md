## Task: `[toolchain]` configuration settings

**Objective**: Add a `[toolchain]` section to `Settings` so users can override the
Flutter install directory, channel, and install method. Phase 2 reads
`channel`, `flutter_install_method`, and `flutter_install_dir`; the
Android/JDK keys are declared now (for Phase 3) but unused this phase.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/config/types.rs`: add `ToolchainSettings` struct and a
  `pub toolchain: ToolchainSettings` field on `Settings`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/settings.rs`: how `Settings` is loaded/serialized
  (serde defaults, atomic save) — confirm the new field round-trips.
- `crates/fdemon-app/src/config/types.rs` `FlutterSettings` (line ~146) as the
  pattern for an `Option`/defaulted sub-struct.

### Details

```rust
/// `[toolchain]` settings controlling the Install Wizard's managed installs.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ToolchainSettings {
    /// Where managed Flutter SDKs are installed.
    /// Default (None) → ~/fvm/versions (shared with the Flutter Version panel).
    pub flutter_install_dir: Option<PathBuf>,
    /// Channel for managed installs.
    pub channel: String,                  // default: "stable"
    /// "git" (default) or "archive".
    pub flutter_install_method: String,   // default: "git"
    /// Android SDK root (Phase 3). Default (None) → ~/.android/sdk or $ANDROID_HOME.
    pub android_sdk_root: Option<PathBuf>,
    /// Android API level for platforms/build-tools (Phase 3).
    pub android_api_level: u32,           // default: 36
    /// cmdline-tools build number override (Phase 3).
    pub cmdline_tools_build: Option<String>,
    /// Explicit JDK 17 directory (Phase 3).
    pub jdk_path: Option<PathBuf>,
}

impl Default for ToolchainSettings {
    fn default() -> Self {
        Self {
            flutter_install_dir: None,
            channel: "stable".to_string(),
            flutter_install_method: "git".to_string(),
            android_sdk_root: None,
            android_api_level: 36,
            cmdline_tools_build: None,
            jdk_path: None,
        }
    }
}
```

Add to `Settings`:

```rust
#[serde(default)]
pub toolchain: ToolchainSettings,
```

Provide a small helper to parse `flutter_install_method` into the daemon's
`InstallMethod` (used by task 09), or keep parsing in the handler — your choice;
document it. Suggested helper on `ToolchainSettings`:

```rust
/// Parse `flutter_install_method` into a daemon InstallMethod (defaults to
/// GitClone for unknown values).
pub fn install_method(&self) -> fdemon_daemon::toolchain::InstallMethod { ... }
```

### Acceptance Criteria

1. `Settings` gains a `toolchain` field that defaults correctly when the section
   is absent from `config.toml`.
2. A `[toolchain]` block with overrides deserializes into the expected values.
3. Round-trip (serialize → deserialize) preserves all fields.
4. `install_method()` maps `"git"`/`"archive"` (case-insensitive) and defaults to
   `GitClone` for anything else.
5. Unit tests cover defaults, override parsing, and round-trip. No clippy warnings.

### Testing

```rust
#[test]
fn test_toolchain_settings_default() { /* channel=="stable", method=="git", api==36 */ }

#[test]
fn test_parse_toolchain_section() {
    let toml = r#"
[toolchain]
channel = "beta"
flutter_install_method = "archive"
android_api_level = 35
"#;
    // assert parsed values
}

#[test]
fn test_settings_without_toolchain_uses_defaults() { /* empty toml → default toolchain */ }

#[test]
fn test_install_method_mapping() { /* "git"→GitClone, "archive"→Archive, "x"→GitClone */ }
```

### Notes

- Mirror the existing `FlutterSettings` serde style exactly (`#[serde(default)]`).
- Declaring the Phase-3 Android keys now avoids a second config migration; they are
  inert until Phase 3 consumes them.
- This task references `fdemon_daemon::toolchain::InstallMethod` (task 01). If you
  prefer to avoid the cross-crate reference in config, return a local enum instead
  and map it in task 09 — note whichever you choose.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/types.rs` | Added `ToolchainSettings` struct, `InstallMethod` local enum, `toolchain` field on `Settings`, and 8 unit tests covering defaults, override parsing, round-trip, and `install_method()` mapping |
| `crates/fdemon-app/src/handler/mod.rs` | Boxed `settings` field in `UpdateAction::PersistSettings` to resolve clippy `large_enum_variant` lint triggered by `Settings` growing with `ToolchainSettings` |
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | Updated `PersistSettings` construction to `Box::new(state.settings.clone())` |
| `crates/fdemon-app/src/handler/settings_handlers.rs` | Updated `PersistSettings` construction to `Box::new(state.settings.clone())` |
| `crates/fdemon-app/src/actions/mod.rs` | Updated two `PersistSettings` test constructions to wrap settings in `Box::new` |

### Notable Decisions/Tradeoffs

1. **Local `InstallMethod` enum**: The task notes allow a local enum if `fdemon_daemon::toolchain::InstallMethod` does not exist yet (it is task 01's deliverable). A local `InstallMethod` was defined in `fdemon-app/src/config/types.rs` to avoid a premature cross-crate dependency. Task 09 (wizard handler) will map from this local enum to the daemon's type when wiring the execution path.

2. **`UpdateAction::PersistSettings` boxing**: Adding `ToolchainSettings` increased `Settings`'s stack size enough to trigger the `clippy::large_enum_variant` lint on `PersistSettings`. The `settings` field was boxed (`Box<Settings>`) as recommended by clippy. All call sites updated; auto-deref means usage code (`&settings`, `settings.field`) is unchanged at all destructuring sites.

3. **`#[serde(default)]` on `ToolchainSettings`**: Mirrors the existing `FlutterSettings` pattern exactly. A missing `[toolchain]` section in `config.toml` is treated as `ToolchainSettings::default()`, preserving backward compatibility for all existing config files.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2682 + others, zero failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Phase-3 Android fields are declared but untested end-to-end**: `android_sdk_root`, `android_api_level`, `cmdline_tools_build`, `jdk_path` are present in the struct and covered by round-trip tests, but no Phase-3 code consumes them yet. This is intentional per the task spec.
</content>
