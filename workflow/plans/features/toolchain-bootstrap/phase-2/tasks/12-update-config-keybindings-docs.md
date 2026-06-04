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

**Status:** Done
**Branch:** worktree-agent-a4f35add0f5fab624

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Added `[toolchain]` section with all keys, defaults, and phase status; added automatic `[flutter] sdk_path` write note; updated Table of Contents; added `[toolchain]` block to the complete config example |
| `docs/KEYBINDINGS.md` | Added `Enter` (Run / Retry Step) binding to the Install Wizard Step List Controls table; removed the stale "not yet available in Phase 1" note |

### Notable Decisions/Tradeoffs

1. **Table format with Phase column**: Added a `Phase` column to the `[toolchain]` property table so users can immediately see which keys are active now versus reserved for Phase 3, without having to read prose notes.
2. **Stale note removal**: The "Step execution is not yet available in Phase 1" caveat note was removed since `Enter` is now wired and functional in Phase 2. Keeping it would have contradicted the new keybinding row.
3. **`flutter_install_dir` type**: Documented as `string` in the table (matching how users would write it in TOML), while the Rust type is `Option<PathBuf>`. This matches the pattern used for other path settings in the same file.

### Testing Performed

- Cross-checked every `[toolchain]` key name and default against `ToolchainSettings` in `crates/fdemon-app/src/config/types.rs` — exact match on all 7 fields.
- Cross-checked `Enter` → `Message::InstallWizardRunSelectedStep` against `handle_key_install_wizard()` in `crates/fdemon-app/src/handler/keys.rs` — confirmed present.
- Confirmed no stale or invented keys; all documented keys match source.

### Risks/Limitations

1. **Phase 3 reserved fields**: `android_sdk_root`, `android_api_level`, `cmdline_tools_build`, `jdk_path` are documented but not yet active. If Phase 3 changes their semantics or defaults, these doc entries will need updating.
</content>
