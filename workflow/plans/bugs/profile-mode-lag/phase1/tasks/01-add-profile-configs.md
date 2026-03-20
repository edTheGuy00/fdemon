## Task: Add Profile Mode Configs to Example App3

**Objective**: Configure example app3 with a profile mode launch config and aggressive DevTools polling settings that mirror the reporter's environment from Issue #25, enabling local reproduction of the lag.

**Depends on**: None

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `example/app3/.fdemon/launch.toml`: Add "Profile (Issue #25)" configuration with `mode = "profile"`
- `example/app3/.fdemon/config.toml`: Add aggressive DevTools polling settings

**Files Read (Dependencies):**
- `workflow/plans/bugs/profile-mode-lag/BUG.md`: Reporter's config for reference

### Details

#### 1. Update `example/app3/.fdemon/launch.toml`

Add a new `[[configurations]]` entry at the **top** of the file with profile mode and `auto_start = true`. Remove `auto_start = true` from the existing "Staging" config so the profile config is the one that auto-launches.

Update the file comment to reflect app3's new dual purpose (Issue #18 multi-config testing + Issue #25 profile lag reproduction).

Target state:

```toml
# Launch configurations for profile mode lag reproduction (Issue #25)
# and multi-config testing (Issue #18).
#
# The "Profile (Issue #25)" config has auto_start = true and mode = "profile".
# Use it to reproduce the lag reported in Issue #25.
# Switch to "Development" (debug mode) for A/B comparison.

[[configurations]]
name = "Profile (Issue #25)"
device = "auto"
mode = "profile"
auto_start = true

[[configurations]]
name = "Development"
device = "auto"

[[configurations]]
name = "Staging"
device = "auto"
flavor = "staging"

[[configurations]]
name = "Production"
device = "auto"
flavor = "production"
```

Key changes:
- New "Profile (Issue #25)" config added first with `mode = "profile"` and `auto_start = true`
- `auto_start = true` removed from "Staging" (Issue #18 is already fixed/merged)
- Comments updated to explain the reproduction purpose

#### 2. Update `example/app3/.fdemon/config.toml`

Add the aggressive DevTools polling settings from the reporter's config. These settings produce maximum VM Service pressure (hitting the code-enforced minimums) to reliably reproduce the lag.

Add the following to the existing `[devtools]` section:

```toml
[devtools]
auto_open = false
browser = ""
default_panel = "performance"
performance_refresh_ms = 500
memory_history_size = 60
tree_max_depth = 0
inspector_fetch_timeout_secs = 60
auto_repaint_rainbow = false
auto_performance_overlay = false
allocation_profile_interval_ms = 1000
max_network_entries = 500
network_auto_record = true
network_poll_interval_ms = 1000

[devtools.logging]
hybrid_enabled = true
prefer_vm_level = true
show_source_indicator = true
dedupe_threshold_ms = 100
```

Also add DAP settings to match the reporter:

```toml
[dap]
enabled = true
auto_start_in_ide = false
port = 33001
bind_address = "127.0.0.1"
suppress_reload_on_pause = true
auto_configure_ide = false
```

Update the file header comment to explain the aggressive settings and their purpose.

### Acceptance Criteria

1. `example/app3/.fdemon/launch.toml` contains a "Profile (Issue #25)" config with `mode = "profile"` and `auto_start = true`
2. `auto_start` is removed from the "Staging" config (no duplicate auto_start)
3. Existing configs (Development, Staging, Production) are preserved with their device/flavor settings
4. `example/app3/.fdemon/config.toml` has all DevTools polling fields set to their minimum allowed values (`performance_refresh_ms = 500`, `allocation_profile_interval_ms = 1000`, `network_poll_interval_ms = 1000`)
5. `default_panel = "performance"` is set (so the performance panel opens by default, triggering the expensive polling immediately)
6. DAP settings match the reporter's config
7. File parses correctly as valid TOML (no syntax errors)

### Testing

- `cargo build --workspace` — confirms no TOML parse errors at compile time (launch.toml is read at runtime, but validates the workspace still builds)
- Manual: `cargo run -- example/app3` — should auto-launch with "Profile (Issue #25)" config in profile mode

### Notes

- The reporter's `config.toml` also includes `[behavior] auto_start = true`, but app3 uses `auto_start = false` at the behavior level, relying on per-config `auto_start` instead. Keep it this way — it's a better test of the per-config auto_start path.
- `performance_refresh_ms = 500` is the code-enforced minimum (`PERF_POLL_MIN_MS` in `actions/performance.rs:28`). Setting it lower would have no effect.
- `allocation_profile_interval_ms = 1000` is the code-enforced minimum (`ALLOC_PROFILE_POLL_MIN_MS` in `actions/performance.rs:35`). This produces the maximum allocation profiling pressure.
- The `network_poll_interval_ms = 1000` value is above the minimum (500ms) but matches the reporter's setting.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a9f98916

### Files Modified

| File | Changes |
|------|---------|
| `example/app3/.fdemon/launch.toml` | Added "Profile (Issue #25)" config at top with `mode = "profile"` and `auto_start = true`; removed `auto_start = true` from "Staging"; updated header comment |
| `example/app3/.fdemon/config.toml` | Added aggressive DevTools polling fields to `[devtools]` section, added `[devtools.logging]` sub-table, added `[dap]` section; updated header comment |

### Notable Decisions/Tradeoffs

1. **Section ordering in config.toml**: Placed `[devtools.logging]` immediately after `[devtools]` (before `[dap]`) to keep the sub-table adjacent to its parent, consistent with TOML conventions.
2. **Preserved all pre-existing keys**: All existing keys in both files are retained; only new content was added.

### Testing Performed

- `cargo build --workspace` - Passed (26s, no errors)

### Risks/Limitations

1. **Runtime-only TOML validation**: The TOML files are parsed at runtime, not at compile time; the build check confirms the workspace compiles, but full validation requires running `cargo run -- example/app3`.
