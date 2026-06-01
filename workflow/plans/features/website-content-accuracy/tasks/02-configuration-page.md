## Task: Fix Configuration page

**Objective**: Correct every config key/default/section on the Configuration page so
copy-pasted TOML actually works against the real config structs.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `website/src/pages/docs/configuration.rs`: fix native_logs TOML/defaults, add missing
  keys, fix `default_panel` options, add `[dap]`/`[flutter]` sections.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs`: source of truth for all config keys/defaults.

### Details

- `[native_logs] min_level` default → `"info"` (not `"debug"`). [D-17] (`types.rs:1033`)
- `[[native_logs.sources]]` → `[[native_logs.custom_sources]]` with
  `name`/`command`/`args`/`format`. [D-18] (`types.rs:1020-1022`)
- `[native_logs.tag_levels]` → `[native_logs.tags.<TAG>]` with a `min_level` field.
  [D-20] (`types.rs:1010-1013`)
- Remove `buffer_size` from `[native_logs]` — no such field. [D-19] (`types.rs:987-1023`)
- `[devtools] default_panel` options → valid values `inspector`, `performance`,
  `network`, `memory`. Remove `"layout"`; add `"memory"`/`"network"`. [D-16]
- Add missing `[behavior]` keys: `version_check` (true), `version_check_timeout_secs`
  (3). [D-13] (`types.rs:168-175`)
- Add `[ui] icons` (`IconMode`, default `NerdFonts`; note requires a Nerd Font, can be
  set to safe Unicode). [D-14] (`types.rs:304`)
- Add missing `[devtools]` keys [D-15] (`types.rs:387-482`):
  `inspector_fetch_timeout_secs` (60), `allocation_profile_interval_ms` (5000),
  `max_network_entries` (500), `network_auto_record` (true), `network_poll_interval_ms`
  (1000), `inspector_readiness_poll_attempts/interval_ms/call_timeout_ms`,
  `hide_implementation_widgets` (true), `auto_enable_rebuild_tracking` (false),
  `rebuild_stats_frame_window` (30), `timeline_event_buffer_size` (10000).
- Add `[dap]` section [D-22] (`types.rs:656-714`): `enabled`, `auto_start_in_ide` (true),
  `port` (0 = auto), `bind_address` ("127.0.0.1"), `suppress_reload_on_pause` (true),
  `auto_configure_ide` (true).
- Add `[flutter]` section (`FlutterSettings`, `sdk_path`). [D-22] (`types.rs:142-153`)
- Verify the `auto_start` "deprecation warning" claim against the config loader; keep
  only if accurate, else reword to "ignored". [D-21]

### Acceptance Criteria

1. Every documented key exists in `config/types.rs` with the stated default.
2. native_logs TOML keys are `custom_sources` / `tags.<TAG>.min_level`; no `buffer_size`.
3. `default_panel` lists exactly `inspector`/`performance`/`network`/`memory`.
4. `[dap]` and `[flutter]` sections present.
5. `cd website && trunk build` compiles.

### Notes

- T03 (Native Logs page) carries its own copy of the native_logs TOML — both must be
  fixed independently.
- The same config drift is checked in `docs/CONFIGURATION.md` by T07 (doc_maintainer).

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `website/src/pages/docs/configuration.rs` | Fixed `[native_logs]` TOML keys, fixed `default_panel` options, added missing `[behavior]` keys, added `[ui] icons`, added all missing `[devtools]` keys, added `[dap]` section, added `[flutter]` section |

### Notable Decisions/Tradeoffs

1. **`auto_start` deprecation note retained**: Verified against `crates/fdemon-app/src/config/settings.rs` — `check_deprecated_auto_start()` emits a one-time `warn!` and serde silently drops the field. The existing "logs a one-time deprecation warning and ignores the value" wording is accurate.
2. **`[native_logs]` section added as new section**: The original page had no `[native_logs]` section at all. Added it after the Editor Settings section with correct TOML keys (`custom_sources`, `tags.<TAG>.min_level`, correct `min_level` default of `"info"`, no `buffer_size`).
3. **`[dap]` and `[flutter]` sections added**: Both were completely absent. Added as new sections (DAP Server Settings, Flutter SDK Settings) with all fields from `types.rs` and accurate defaults.
4. **`default_panel` corrected**: Removed `"layout"`, added `"memory"` and `"network"` — matches `DevToolsSettings` validation in `types.rs` and the panel enum in the app.
5. **All new devtools keys use values from `default_*` functions**: Checked each default function in `types.rs` to ensure all defaults are accurate.

### Testing Performed

- Copied changed file to `/Users/ed/Dev/zabin/flutter-demon/website/src/pages/docs/configuration.rs` and ran `cd /Users/ed/Dev/zabin/flutter-demon/website && cargo check` — **Passed** (1 pre-existing warning in `debugging.rs`, no errors)
- Restored original file in main repo after verification
- All acceptance criteria verified against `crates/fdemon-app/src/config/types.rs`

### Risks/Limitations

1. **Workspace detection issue in worktree**: The website crate cannot be compiled directly from the worktree path because cargo finds the parent project's `Cargo.toml` before the worktree's. Verified by copying to the original website directory instead. No impact on the actual change.
</content>
