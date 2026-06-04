## Task: Update CONFIGURATION.md + KEYBINDINGS.md for Phase 3

**Objective**: Document the now-active `[toolchain]` Android/JDK config keys and the
new `c` (copy guided command) wizard keybinding.

**Depends on**: 04

**Agent:** implementor

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `docs/CONFIGURATION.md`: document the `[toolchain]` Android/JDK keys now consumed
  by Phase 3: `android_sdk_root`, `android_api_level` (default 36),
  `cmdline_tools_build` (override for the cmdline-tools download URL build number),
  `jdk_path`. Note that `android_sdk_root` is written automatically after a
  successful Android Tools install.
- `docs/KEYBINDINGS.md`: add the `c` key for the Install Wizard
  (copy the selected step's guided command to the clipboard); confirm `Enter` now
  also runs the Android Tools step (gated on JDK 17).

**Files Read (Dependencies):**
- task file 04 (message/keybinding additions).
- `crates/fdemon-app/src/config/types.rs` (`ToolchainSettings` field names/defaults).

### Details

CONFIGURATION.md — under the `[toolchain]` section (Phase 2 added the Flutter keys),
document the Android/JDK keys:

```toml
[toolchain]
# Android SDK root (default: $ANDROID_HOME / $ANDROID_SDK_ROOT, else the per-OS
# default: ~/Android/Sdk, ~/Library/Android/sdk, or %LOCALAPPDATA%\Android\Sdk).
# Written automatically after a successful Android Tools install.
# android_sdk_root = "~/.android/sdk"

# Android API level for platforms/build-tools (default: 36).
android_api_level = 36

# cmdline-tools build number used in the download URL. Override only if the default
# 404s (find the current value on https://developer.android.com/studio#command-tools).
# cmdline_tools_build = "11076708"

# Explicit JDK 17 directory, passed to `flutter config --jdk-dir`.
# jdk_path = "/usr/lib/jvm/java-17-openjdk"
```

KEYBINDINGS.md — Install Wizard section:

| Key | Action |
|-----|--------|
| `c` | Copy the selected step's guided command (e.g. the JDK install command) |

Confirm/extend the `Enter` row to note it runs the Android Tools step (gated on a
present JDK 17), and that `r` re-runs preflight (used after a guided JDK install).

### Acceptance Criteria

1. CONFIGURATION.md documents all four Android/JDK `[toolchain]` keys with defaults
   and the auto-write note for `android_sdk_root`.
2. KEYBINDINGS.md lists the `c` wizard key and reflects `Enter` running the gated
   Android Tools step.
3. Values/defaults match `ToolchainSettings` in `config/types.rs` exactly.
4. Edits are targeted; no unrelated sections changed.

### Notes

- These docs are implementor-editable (not doc_maintainer-managed).
- Keep the build-number guidance honest: there is no stable build-less URL, so the
  default may drift — the override key is the escape hatch.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Updated `[toolchain]` section: replaced Phase 3 (reserved) TOML comments and table with active documentation; added auto-write note for `android_sdk_root`; added `cmdline_tools_build` override guidance with link to developer.android.com; removed "Phase" column from table; descriptions match `ToolchainSettings` defaults exactly (api_level=36, None defaults for optional fields). |
| `docs/KEYBINDINGS.md` | Added `c` (Copy Guided Command) row to Step List Controls; updated `Enter` description to mention Android Tools step gated on JDK 17; extended `r` description to note use after guided JDK install. |

### Notable Decisions/Tradeoffs

1. **`cmdline_tools_build` default label**: Used "(current)" rather than "(latest)" in the table to reflect the task note that the default may drift — the override key is the escape hatch. The TOML comment example uses a concrete build number (`11076708`) as a realistic placeholder.
2. **`android_sdk_root` platform defaults**: The table Default column lists all three per-OS paths for clarity, matching the TOML block comment which also lists all three.
3. **Removed "Phase" column**: The Phase column was removed from the table since all fields are now active in Phase 3; the section header prose provides sufficient context.

### Testing Performed

- Visual review of both edited sections confirms values match `ToolchainSettings::default()` in `config/types.rs` (api_level=36, None for optional fields).
- No source code changes — documentation-only task.

### Risks/Limitations

1. **`cmdline_tools_build` example value**: The example build number `11076708` in the TOML comment block is illustrative and will drift over time. The text directs users to check the official URL for the current value.
