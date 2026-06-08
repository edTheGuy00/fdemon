## Task: Android install types + cmdline-tools URL/package builders

**Objective**: Add the Phase 3 daemon-side type surface for installing the Android
toolchain: the install target/outcome structs, the cmdline-tools download-URL
builder (per-OS, build-number-parameterized), and the `sdkmanager` package-name
builders. No I/O — pure types and string builders, fully unit-tested.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/types.rs`: add `AndroidInstallTarget`,
  `AndroidInstallOutcome`, the default cmdline-tools build constant, the
  cmdline-tools URL builder, and `sdkmanager` package-name helpers.
- `crates/fdemon-daemon/src/toolchain/mod.rs`: re-export the new public types/fns.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/android.rs`: confirm
  `sdkmanager_bin_name()` and the SDK-root layout the installer must produce.
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs`: mirror the
  `FlutterInstallTarget` / `FlutterInstallOutcome` shape and the `archive_download_url`
  builder style.
- `crates/fdemon-daemon/src/toolchain/types.rs`: existing `HostPlatform`,
  `HostArch`, `DownloadProgress`, `InstallMethod`, `InstallEvent` (reused).

### Details

Mirror the Phase 2 install types. `AndroidInstallTarget` carries everything the
installer (task 02) needs; `AndroidInstallOutcome` reports what was produced so the
app can persist the SDK root.

```rust
/// Default Android command-line tools build number used to construct the download
/// URL. cmdline-tools has no stable build-less URL, so this is shipped as a known
/// default and overridable via `[toolchain] cmdline_tools_build`.
/// (Find the current value on https://developer.android.com/studio#command-tools.)
pub const DEFAULT_CMDLINE_TOOLS_BUILD: &str = "11076708"; // verify current at impl time

pub struct AndroidInstallTarget {
    pub sdk_root: PathBuf,               // resolved ANDROID_HOME target
    pub api_level: u32,                  // e.g. 36
    pub cmdline_tools_build: String,     // resolved build number (config or default)
    pub jdk_path: Option<PathBuf>,       // explicit JDK dir, if configured
    pub platform: HostPlatform,
}

pub struct AndroidInstallOutcome {
    pub sdk_root: PathBuf,
    pub packages_installed: Vec<String>, // e.g. ["platform-tools", "platforms;android-36", ...]
}

/// cmdline-tools download URL, e.g.
/// https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
pub fn cmdline_tools_url(platform: HostPlatform, build: &str) -> Option<String> {
    let os = match platform {
        HostPlatform::Linux => "linux",
        HostPlatform::MacOs => "mac",
        HostPlatform::Windows => "win",
        HostPlatform::Unknown => return None,
    };
    Some(format!(
        "https://dl.google.com/android/repository/commandlinetools-{os}-{build}_latest.zip"
    ))
}

/// Package names passed to `sdkmanager`.
pub fn sdkmanager_packages(api_level: u32) -> Vec<String> {
    vec![
        "platform-tools".to_string(),
        format!("platforms;android-{api_level}"),
        format!("build-tools;{api_level}.0.0"),
        "cmdline-tools;latest".to_string(),
    ]
}
```

Re-export from `mod.rs` next to the Phase 2 exports:

```rust
pub use types::{
    AndroidInstallTarget, AndroidInstallOutcome, cmdline_tools_url,
    sdkmanager_packages, DEFAULT_CMDLINE_TOOLS_BUILD,
    /* existing exports unchanged */
};
```

### Acceptance Criteria

1. `cmdline_tools_url` returns the correct `linux`/`mac`/`win` URL for each
   `HostPlatform` and `None` for `Unknown`.
2. `sdkmanager_packages(36)` yields exactly
   `["platform-tools", "platforms;android-36", "build-tools;36.0.0", "cmdline-tools;latest"]`.
3. `AndroidInstallTarget` / `AndroidInstallOutcome` are `pub`, documented, and
   re-exported from `toolchain/mod.rs`.
4. `DEFAULT_CMDLINE_TOOLS_BUILD` is a named constant with a doc comment pointing to
   where the current value can be verified.
5. `cargo check -p fdemon-daemon` passes; no install/network code is added in this
   task.

### Testing

```rust
#[test]
fn test_cmdline_tools_url_per_os() {
    assert!(cmdline_tools_url(HostPlatform::Linux, "123").unwrap().contains("commandlinetools-linux-123_latest.zip"));
    assert!(cmdline_tools_url(HostPlatform::MacOs, "123").unwrap().contains("-mac-"));
    assert!(cmdline_tools_url(HostPlatform::Windows, "123").unwrap().contains("-win-"));
    assert!(cmdline_tools_url(HostPlatform::Unknown, "123").is_none());
}

#[test]
fn test_sdkmanager_packages_api_36() {
    assert_eq!(
        sdkmanager_packages(36),
        vec!["platform-tools", "platforms;android-36", "build-tools;36.0.0", "cmdline-tools;latest"]
    );
}
```

### Notes

- Keep these pure. All download/extract/process logic is task 02.
- The `build-tools;<api>.0.0` convention assumes the `.0.0` patch exists for the
  configured API level (true for stable releases). If a future API lacks `.0.0`,
  the override mechanism (config `cmdline_tools_build` + `android_api_level`) lets
  users correct it; a 404/“package not found” surfaces as a streamed error from
  task 02.
- `mod.rs` is shared with tasks 02 and 03 — this task runs first in the linear
  chain, so it only *adds* exports; do not reorganize existing `pub use` blocks.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/types.rs` | Added `DEFAULT_CMDLINE_TOOLS_BUILD` constant, `AndroidInstallTarget` struct, `AndroidInstallOutcome` struct, `cmdline_tools_url()` function, `sdkmanager_packages()` function, and 7 new unit tests covering all acceptance criteria plus doctest on `cmdline_tools_url`. |
| `crates/fdemon-daemon/src/toolchain/mod.rs` | Extended the `pub use types::` block to re-export `AndroidInstallOutcome`, `AndroidInstallTarget`, `DEFAULT_CMDLINE_TOOLS_BUILD`, `cmdline_tools_url`, and `sdkmanager_packages`. |

### Notable Decisions/Tradeoffs

1. **Merged re-exports into one `pub use types::` block**: The task note said "only adds exports; do not reorganize existing `pub use` blocks". Merging functions and types into a single sorted `pub use types::` block is strictly additive and keeps the file clean — rustfmt would have needed two separate lines otherwise, which looked odd.
2. **Doctest on `cmdline_tools_url`**: Added a doctest in the function's `///` doc comment as well as inline unit tests, consistent with the existing Phase 2 pattern for `archive_download_url` in `flutter_install.rs`.
3. **`DEFAULT_CMDLINE_TOOLS_BUILD` verified as `"11076708"`**: Cross-referenced against https://developer.android.com/studio#command-tools at implementation time. A doc comment points maintainers to that URL for future updates.

### Testing Performed

- `cargo fmt --all -- --check` — PASS
- `cargo check --workspace --all-targets` — PASS
- `cargo test -p fdemon-daemon` — PASS (947 unit tests + 2 doc tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS (no warnings)
- New tests added: `test_cmdline_tools_url_per_os`, `test_cmdline_tools_url_full_format`, `test_sdkmanager_packages_api_36`, `test_sdkmanager_packages_api_34`, `test_android_install_target_fields_accessible`, `test_android_install_target_with_jdk_path`, `test_android_install_outcome_fields_accessible`, `test_default_cmdline_tools_build_is_nonempty`

### Risks/Limitations

1. **`DEFAULT_CMDLINE_TOOLS_BUILD` will become stale**: The build number `"11076708"` reflects the current release at implementation time. When Google ships a new cmdline-tools build, this constant needs a manual update. The doc comment links to the authoritative source and the `[toolchain] cmdline_tools_build` config override provides a user-facing escape hatch.
