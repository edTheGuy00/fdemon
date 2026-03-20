## Task: Add Profile Mode Lag Reproduction Test to TESTING.md

**Objective**: Document the manual test procedure for reproducing the profile mode lag (Issue #25) in `example/TESTING.md`, following the existing test format.

**Depends on**: None

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `example/TESTING.md`: Add new test section for profile mode lag reproduction

**Files Read (Dependencies):**
- `example/app3/.fdemon/launch.toml`: Reference the new profile config
- `example/app3/.fdemon/config.toml`: Reference the aggressive polling settings
- `workflow/plans/bugs/profile-mode-lag/BUG.md`: Issue context

### Details

Add a new test section to `example/TESTING.md` following the existing pattern (Test A through Test H). The new test should be **Test I** and cover:

1. Starting app3 (which auto-launches with the profile config)
2. Observing the lag when the performance panel is active
3. A/B comparison: switching to the "Development" config (debug mode) with identical DevTools settings to confirm the lag is profile-mode-specific

#### Test to add (append after Test H):

```markdown
## Test I — Verify profile mode lag reproduction (app3, Issue #25)

**Purpose**: Reproduce the profile mode lag reported in Issue #25. Aggressive
DevTools polling settings cause visible freezes in profile mode but not in
debug mode, confirming the root cause is VM Service polling pressure.

**Configuration:** `example/app3/.fdemon/config.toml`
```toml
[devtools]
performance_refresh_ms = 500
allocation_profile_interval_ms = 1000
network_poll_interval_ms = 1000
network_auto_record = true
default_panel = "performance"
```

`example/app3/.fdemon/launch.toml`
```toml
[[configurations]]
name = "Profile (Issue #25)"
device = "auto"
mode = "profile"
auto_start = true
```

**Steps:**

1. Start app3:
   ```
   cargo run -- example/app3
   ```
2. The "Profile (Issue #25)" config auto-launches in profile mode.
3. Once the app is running, press `D` to enter DevTools mode — the performance
   panel opens by default (`default_panel = "performance"`).
4. Observe the running Flutter app on the device — look for periodic freezes
   (~1 second apart), matching the `allocation_profile_interval_ms = 1000` cadence.
5. Press `Q` to quit the session.
6. Start a new session (`N`), select "Development" (debug mode), and repeat
   steps 3-4.
7. Compare: the same DevTools polling settings should produce no visible lag
   in debug mode.

**Expected result**: Profile mode shows periodic freezes when DevTools polling
is active; debug mode does not. This confirms the lag is caused by VM Service
polling pressure in profile mode.
```

Also update the **Directory Structure Reference** at the bottom of TESTING.md to reflect app3's new purpose:

```
├── app3/                  # Profile mode lag reproduction (Issue #25) + multi-config (Issue #18)
│   ├── .fdemon/
│   │   ├── config.toml    # Aggressive DevTools polling (min intervals)
│   │   └── launch.toml    # "Profile (Issue #25)" with auto_start + mode=profile
│   └── lib/
```

### Acceptance Criteria

1. `example/TESTING.md` contains a new "Test I" section for profile mode lag reproduction
2. The test follows the existing format (Purpose, Configuration, Steps, Expected result)
3. The test references the correct config file paths and settings
4. Steps include both the profile mode reproduction AND the debug mode A/B comparison
5. The directory structure reference is updated to reflect app3's new dual purpose
6. No existing tests (A through H) are modified

### Testing

- Read through the test procedure and verify it matches the actual config values in app3's `.fdemon/` files
- Manual: follow the test steps on a real device to confirm reproducibility (optional — depends on available hardware)

### Notes

- The test is lettered "Test I" (continuing from the existing Test H)
- The A/B comparison step (debug vs profile with same settings) is important — it isolates the variable (build mode) and confirms the root cause hypothesis from BUG.md
- The test references pressing `D` for DevTools mode and `N` for new session — these are fdemon's standard keybindings
