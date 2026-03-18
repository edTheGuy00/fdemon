# Flutter Demon — Manual Testing Guide

This document describes manual test procedures for the example apps in this
directory. The tests cover watcher path resolution (Issue #17) and per-config
`auto_start` behaviour (Issue #18).

## Prerequisites

- Rust toolchain installed (`cargo` in PATH)
- A connected device or running emulator (or `device = "auto"` will select one)
- Run all commands from the repository root

---

## Test A — Verify default watcher paths (app2, baseline)

**Purpose**: Confirm that the simplest `paths = ["lib"]` watcher works correctly
before testing more complex scenarios.

**Steps:**

1. Start app2:
   ```
   cargo run -- example/app2
   ```
2. In fdemon's log output, look for lines containing `Watching:` — confirm
   `example/app2/lib` appears.
3. Edit any `.dart` file under `example/app2/lib/` and save.
4. Verify fdemon triggers hot reload (log entry "Hot reload" or similar).

**Expected result**: Single `lib/` path is watched; hot reload fires on save.

---

## Test B — Verify cross-project watcher paths (app1, `../app2/lib`)

**Purpose**: Confirm that a relative path pointing to a sibling project's lib
directory (`../app2/lib`) is resolved and watched correctly.

**Configuration:** `example/app1/.fdemon/config.toml`
```toml
[watcher]
paths = ["lib", "../app2/lib"]
```

**Steps:**

1. Start app1:
   ```
   cargo run -- example/app1
   ```
2. In fdemon's output, look for `Watching:` lines — confirm both
   `example/app1/lib` and `example/app2/lib` appear.
3. Edit a `.dart` file under `example/app2/lib/` and save.
4. Verify fdemon triggers hot reload.

**Expected result**: Both paths are resolved and watched; editing app2's lib
triggers reload in app1's session.

---

## Test C — Verify `../` watcher paths (app4, `../shared_lib`)

**Purpose**: Confirm that a relative path using `..` traversal resolves
correctly to `example/shared_lib/` from `example/app4/`. Also confirms that a
permanently configured non-existent path produces a warning without crashing
(see Test G).

**Configuration:** `example/app4/.fdemon/config.toml`
```toml
[watcher]
paths = ["lib", "../shared_lib", "../app1/lib", "../nonexistent"]
```

Note: `../nonexistent` does not exist on disk and is kept in the config
intentionally to exercise non-existent path warning behavior.

**Steps:**

1. Start app4:
   ```
   cargo run -- example/app4
   ```
2. Confirm fdemon logs show `Watching:` for the three valid paths:
   - `example/app4/lib`
   - `example/shared_lib`
   - `example/app1/lib`
3. Confirm fdemon logs a warning for `../nonexistent` (path does not exist).
4. Edit `example/shared_lib/shared_utils.dart` and save.
5. Verify fdemon triggers hot reload.

**Expected result**: `../shared_lib` resolves to `example/shared_lib/`;
editing the shared file triggers reload. A warning is logged for the
non-existent path but fdemon does not crash.

---

## Test D — Verify auto_start with single config (app1)

**Purpose**: Confirm that `auto_start = true` on a launch config causes fdemon
to skip the device selection dialog and immediately start the app.

**Configuration:** `example/app1/.fdemon/launch.toml`
```toml
[[configurations]]
name = "App1 | Default"
device = "auto"
auto_start = true
```

**Steps:**

1. Start app1:
   ```
   cargo run -- example/app1
   ```
2. Observe the startup sequence — fdemon should NOT show the NewSessionDialog.
3. The app should begin launching automatically with the "App1 | Default" config.

**Expected result**: No device selection dialog; flutter launches immediately.

---

## Test E — Verify auto_start with multiple configs (app3)

**Purpose**: Confirm that when multiple configs exist and only one has
`auto_start = true`, fdemon correctly selects that specific config
(not the first one, not all of them).

**Configuration:** `example/app3/.fdemon/launch.toml`
```toml
[[configurations]]
name = "Development"
device = "auto"

[[configurations]]
name = "Staging"
device = "auto"
auto_start = true
flavor = "staging"

[[configurations]]
name = "Production"
device = "auto"
flavor = "production"
```

**Steps:**

1. Start app3:
   ```
   cargo run -- example/app3
   ```
2. Observe the startup sequence — fdemon should skip the dialog.
3. In fdemon's status bar or log output, confirm the session launched with the
   "Staging" config (e.g., `--flavor staging` visible in the launch command).

**Expected result**: "Staging" config auto-launches; "Development" and
"Production" do not launch.

---

## Test F — Verify no auto_start shows dialog (app2)

**Purpose**: Confirm that without `auto_start`, fdemon always shows the
NewSessionDialog on startup.

**Configuration:** `example/app2/.fdemon/config.toml`
```toml
[behavior]
auto_start = false
```
`example/app2/.fdemon/launch.toml` — no `auto_start` on any config.

**Steps:**

1. Start app2:
   ```
   cargo run -- example/app2
   ```
2. Observe the startup sequence — the NewSessionDialog (device/config selector)
   should appear.

**Expected result**: Device selection dialog is shown; no auto-launch.

---

## Test G — Verify non-existent watcher path warning (app4, bad path)

**Purpose**: Confirm that fdemon logs a warning (and does not crash) when a
watcher path does not exist on disk.

**Note**: `example/app4/.fdemon/config.toml` already includes `"../nonexistent"`
as a permanent entry for this purpose — no config edits are needed.

**Steps:**

1. Start app4:
   ```
   cargo run -- example/app4
   ```
2. Check fdemon log output for a warning about `../nonexistent`.
3. Verify fdemon still starts and watches the three valid paths (`lib`,
   `../shared_lib`, `../app1/lib`).

**Expected result**: A warning is logged for `../nonexistent`; the other
three paths are watched normally; fdemon does not crash.

---

## Test H — Verify custom extensions (app4, `.json` files)

**Purpose**: Confirm that `extensions = ["dart", "json"]` causes fdemon to
also trigger hot reload when `.json` files change.

**Configuration:** `example/app4/.fdemon/config.toml`
```toml
[watcher]
extensions = ["dart", "json"]
```

**Steps:**

1. Start app4:
   ```
   cargo run -- example/app4
   ```
2. Create or edit a `.json` file inside any watched directory, e.g.:
   ```
   echo '{"version": 2}' > example/app4/lib/config.json
   ```
3. Verify fdemon triggers hot reload for the `.json` change.
4. Confirm a `.txt` file change does NOT trigger reload:
   ```
   echo 'hello' > example/app4/lib/notes.txt
   ```

**Expected result**: `.json` file changes trigger reload; `.txt` changes do not.

---

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

---

## Directory Structure Reference

```
example/
├── app1/                  # Has auto_start + cross-project watcher paths
│   ├── .fdemon/
│   │   ├── config.toml    # paths = ["lib", "../app2/lib"]
│   │   └── launch.toml    # auto_start = true on first config
│   └── lib/
├── app2/                  # Baseline: no auto_start, default paths
│   ├── .fdemon/
│   │   ├── config.toml    # auto_start = false, paths = ["lib"]
│   │   └── launch.toml    # no auto_start
│   └── lib/
├── app3/                  # Profile mode lag reproduction (Issue #25) + multi-config (Issue #18)
│   ├── .fdemon/
│   │   ├── config.toml    # Aggressive DevTools polling (min intervals)
│   │   └── launch.toml    # "Profile (Issue #25)" with auto_start + mode=profile
│   └── lib/
├── app4/                  # Watcher path edge cases (Issue #17 reproduction)
│   ├── .fdemon/
│   │   ├── config.toml    # paths = ["lib", "../shared_lib", "../app1/lib", "../nonexistent"]
│   │   └── launch.toml    # single config, no auto_start
│   └── lib/
└── shared_lib/            # Shared Dart code — NOT a Flutter project
    └── shared_utils.dart  # Edit this to test ../ watcher resolution
```
