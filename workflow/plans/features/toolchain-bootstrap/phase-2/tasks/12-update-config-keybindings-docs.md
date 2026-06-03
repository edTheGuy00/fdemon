## Task: Update CONFIGURATION.md + KEYBINDINGS.md for Phase 2

**Objective**: Document the new `[toolchain]` config section (and the
auto-written `[flutter] sdk_path`) and the `Enter` key in the Install Wizard.

**Depends on**: 05, 06

**Agent:** implementor (these docs are implementor-editable)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `docs/CONFIGURATION.md`: add a `[toolchain]` section reference; note that
  `[flutter] sdk_path` is written automatically after a managed Flutter install.
- `docs/KEYBINDINGS.md`: add the Install Wizard `Enter` binding (run/retry the
  selected step).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs` (task 06: `ToolchainSettings` fields +
  defaults) for accurate key names and defaults.
- `crates/fdemon-app/src/handler/keys.rs` (task 05) for the exact wizard keys.

### Details

`[toolchain]` documentation should list each key, its default, and whether it is
active in Phase 2 or reserved for Phase 3:

```toml
[toolchain]
# Where managed Flutter SDKs are installed (default: ~/fvm/versions/<version>)
# flutter_install_dir = "~/fvm/versions"
channel = "stable"                  # Phase 2
flutter_install_method = "git"      # Phase 2: "git" (default) or "archive"
# android_sdk_root = "~/.android/sdk"   # Phase 3 (reserved)
android_api_level = 36                  # Phase 3 (reserved)
# cmdline_tools_build = "..."           # Phase 3 (reserved)
# jdk_path = "/usr/lib/jvm/java-17-openjdk"  # Phase 3 (reserved)
```

Note: existing `[flutter] sdk_path` is written automatically after a managed
install so fdemon resolves the new SDK without a restart.

KEYBINDINGS.md — under the Install Wizard section, document the full current set
(Esc close, Tab switch pane, j/k ↑/↓ navigate/scroll, r re-run preflight) plus the
Phase 2 addition:

| Key | Action |
|-----|--------|
| `Enter` | Run / retry the selected step (Flutter SDK or PATH Config) |

### Acceptance Criteria

1. CONFIGURATION.md documents every `[toolchain]` key with default and phase
   status, matching `ToolchainSettings`.
2. CONFIGURATION.md notes the automatic `[flutter] sdk_path` write.
3. KEYBINDINGS.md lists the wizard `Enter` binding and is consistent with the
   actual key handlers.
4. No stale or invented keys; cross-check against `keys.rs`.

### Notes

- Keep these docs descriptive; do not duplicate architecture content (that's
  ARCHITECTURE.md, task 11).
- If a dedicated Install Wizard section doesn't yet exist in KEYBINDINGS.md, add
  one mirroring the Flutter Version panel section's format.

---

## Completion Summary

**Status:** Not Started
</content>
