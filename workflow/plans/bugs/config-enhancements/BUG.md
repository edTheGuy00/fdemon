# Bugfix Plan: Config Enhancements

## TL;DR

Two configuration bugs: (1) custom watcher paths from `config.toml` (including relative `../../`) are silently ignored because `settings.watcher.paths` is never passed to `WatcherConfig` — the watcher always uses the hardcoded default `["lib"]`. (2) `auto_start = true` in `launch.toml` has no effect because the TUI startup unconditionally shows the NewSessionDialog — the existing `Message::StartAutoLaunch` infrastructure is never triggered. Both fixes are straightforward wiring issues; the underlying infrastructure already exists.

## Bug Reports

### Bug 1: Watcher Paths Ignored (Issue #17)

**Symptom:** User adds `../../` (or any custom path) to `[watcher].paths` in `.fdemon/config.toml`, but the watcher only watches the default `lib/` directory. Hot reload never triggers for changes in the configured paths.

**Expected:** The file watcher should watch all paths listed in `config.toml`, resolving relative paths (including `../`) against the project root, and canonicalizing them before passing to the `notify` crate.

**Root Cause Analysis:**

1. `config.toml` is correctly parsed into `WatcherSettings.paths: Vec<String>` (`config/types.rs:158-161`).
2. `Engine::start_file_watcher` (`engine.rs:748-753`) builds a `WatcherConfig::new()` (which defaults to `["lib"]`) and only passes `.with_debounce_ms()` and `.with_auto_reload()` — **it never calls `.with_paths()` or `.with_extensions()`**.
3. Therefore `settings.watcher.paths` and `settings.watcher.extensions` from `config.toml` are silently dropped.
4. Secondary issue: even if paths were passed through, `run_watcher` (`watcher/mod.rs:217-228`) does `project_root.join(relative_path)` with no `canonicalize()`, which produces paths like `/project/root/../../` — syntactically valid but unreliable with `notify` backends (especially `kqueue` on macOS).

**Affected Files:**
- `crates/fdemon-app/src/engine.rs:748-753` — missing `.with_paths()` and `.with_extensions()` calls
- `crates/fdemon-app/src/watcher/mod.rs:217-228` — needs `canonicalize()` before `debouncer.watch()`

---

### Bug 2: auto_start in launch.toml Ignored (Issue #18)

**Symptom:** User has multiple configurations in `.fdemon/launch.toml` with one having `auto_start = true`, but fdemon always starts on the NewSessionDialog.

**Expected:** When a configuration has `auto_start = true`, fdemon should skip the dialog and immediately discover devices then launch with that configuration.

**Root Cause Analysis:**

1. `launch.toml` is parsed correctly; `LaunchConfig.auto_start: bool` exists (`config/types.rs:14-47`).
2. The full auto-launch infrastructure exists and is tested:
   - `get_first_auto_start()` (`priority.rs:93-95`) finds configs with `auto_start = true`
   - `Message::StartAutoLaunch` (`message.rs:401-406`) triggers the auto-launch flow
   - `spawn_auto_launch()` (`spawn.rs:134-203`) discovers devices and finds the right config
   - `Message::AutoLaunchResult` handler (`handler/update.rs:870-935`) creates the session
3. **The trigger is never sent.** `startup_flutter()` (`tui/startup.rs:22-36`) unconditionally calls `state.show_new_session_dialog(configs)` and sets `UiMode::Startup` — it never checks for `auto_start` configs.
4. `run_with_project()` (`tui/runner.rs:22-55`) calls `startup_flutter()` but never sends `Message::StartAutoLaunch`.
5. A test at `startup.rs:56-68` explicitly documents and enforces this broken behavior.

**Affected Files:**
- `crates/fdemon-tui/src/startup.rs:22-36` — needs conditional branch for auto_start
- `crates/fdemon-tui/src/runner.rs:22-55` — needs to send `StartAutoLaunch` when auto_start config found
- `crates/fdemon-tui/src/startup.rs:56-68` — test that enforces broken behavior needs updating

---

## Affected Modules

- `crates/fdemon-app/src/engine.rs`: Pass `settings.watcher.paths` and `settings.watcher.extensions` to `WatcherConfig`
- `crates/fdemon-app/src/watcher/mod.rs`: Canonicalize paths before passing to `notify` debouncer
- `crates/fdemon-tui/src/startup.rs`: Add conditional auto-start branch, update tests
- `crates/fdemon-tui/src/runner.rs`: Wire `StartAutoLaunch` message when auto_start config found

---

## Phases

### Phase 1: Fix Watcher Path Pass-through (Bug #17)

Wire `settings.watcher.paths` and `settings.watcher.extensions` into `WatcherConfig` in `Engine::start_file_watcher`, and add path canonicalization in `run_watcher`.

**Steps:**

1. **Pass settings to WatcherConfig** — In `engine.rs:748-753`, add `.with_paths()` converting `settings.watcher.paths` to `Vec<PathBuf>` and `.with_extensions(settings.watcher.extensions.clone())`.
2. **Canonicalize paths in run_watcher** — In `watcher/mod.rs:217-228`, after `project_root.join(relative_path)`, call `.canonicalize()` (with fallback to the joined path if it doesn't exist yet). This handles `../../` and other relative components.
3. **Handle absolute vs relative paths** — If the path from config is already absolute, don't re-join with `project_root`. Use `path.is_absolute()` check.
4. **Add unit tests** — Test path resolution for `lib`, `../../shared`, `../common/lib`, absolute paths, and non-existent paths.

**Measurable Outcomes:**
- Custom paths from `config.toml` are actually watched
- `../../` resolves to the correct canonical path
- Existing default behavior (`["lib"]`) is preserved when no custom paths configured
- `settings.watcher.extensions` is respected

---

### Phase 2: Fix auto_start Launch (Bug #18)

Wire the existing `StartAutoLaunch` message flow into the TUI startup sequence.

**Steps:**

1. **Modify startup_flutter()** — After loading configs, check if any config has `auto_start = true` using `get_first_auto_start()`. If so, return a new `StartupAction::AutoStart { configs }` variant instead of always showing the dialog.
2. **Handle new StartupAction in runner** — In `run_with_project()`, match on the startup result: if `AutoStart`, send `Message::StartAutoLaunch { configs }` via `engine.msg_sender()`. If `Ready`, keep current behavior (show dialog).
3. **Also check settings.behavior.auto_start** — If `settings.behavior.auto_start == true`, also trigger auto-launch even without a specific launch config marked.
4. **Update tests** — Fix the test at `startup.rs:56-68` to verify auto_start is respected. Add tests for: single auto_start config, multiple configs with one auto_start, no auto_start configs, behavior.auto_start without launch config.

**Measurable Outcomes:**
- `auto_start = true` in `launch.toml` causes fdemon to skip the dialog and auto-launch
- When auto-launch fails (no devices), falls back to showing the dialog with error
- `settings.behavior.auto_start = true` in `config.toml` also triggers auto-launch
- Without any auto_start, the dialog is shown as before

---

### Phase 3: Example Apps and Manual Testing

Create example configurations in `/Users/ed/Dev/zabin/flutter-demon/example/` for manual testing of both fixes.

**Steps:**

1. **Update app1 config** — Add custom watcher paths including `../../` and `../app2/lib` to exercise relative path resolution. Add `auto_start = true` to one launch config.
2. **Update app2 config** — Keep as baseline with default watcher paths and no auto_start.
3. **Create app3 (multi-config auto_start)** — New Flutter app fixture with multiple launch configs, one having `auto_start = true`, to reproduce the exact Issue #18 scenario.
4. **Create app4 (watcher edge cases)** — New Flutter app fixture with various watcher path configurations: absolute paths, `../../`, `../shared`, non-existent paths, empty paths list. Also configure custom extensions.
5. **Add a shared lib directory** — Create `example/shared_lib/` with a sample `.dart` file that `app4`'s watcher should detect changes in via `../../shared_lib`.
6. **Document test scenarios** — Add a `example/TESTING.md` with step-by-step manual test procedures for both bugs.

**Measurable Outcomes:**
- All example apps can be used with `cargo run -- example/appN` for manual testing
- Each configuration scenario exercises specific edge cases
- Test procedures are documented for reproducing and verifying both fixes

---

## Edge Cases & Risks

### Path Resolution
- **Risk:** `canonicalize()` fails if path doesn't exist at startup time (e.g., a directory that gets created later)
- **Mitigation:** Fall back to the raw joined path; the watcher already logs a warning for non-existent paths

### Absolute Paths in config.toml
- **Risk:** User provides absolute path like `/home/user/shared` — `project_root.join()` would still work (absolute paths override the base in `PathBuf::join`)
- **Mitigation:** Explicitly check `is_absolute()` and skip joining; add test coverage

### Auto-start with No Devices
- **Risk:** Auto-launch fires but no devices are connected — user sees a loading screen forever
- **Mitigation:** The existing `AutoLaunchResult` handler already falls back to showing the dialog with an error message. Verify this works with a test.

### Two auto_start Fields
- **Risk:** `config.toml`'s `behavior.auto_start` and `launch.toml`'s per-config `auto_start` could conflict
- **Mitigation:** Define clear precedence: if any `launch.toml` config has `auto_start = true`, use it; otherwise fall back to `behavior.auto_start` as a generic "auto-start with first available device" flag

---

## Further Considerations

1. **Watcher restart on config change?** Currently the watcher is created once at startup. If a user edits `config.toml` while fdemon is running, the watcher won't pick up new paths. This is acceptable for now — document as known limitation.

2. **Multiple auto_start configs?** If multiple configs in `launch.toml` have `auto_start = true`, `get_first_auto_start()` picks the first one. This matches intuitive behavior but should be documented.

---

## Task Dependency Graph

```
Phase 1 (Watcher)          Phase 2 (Auto-start)
├── 01-fix-watcher-paths   ├── 03-fix-auto-start
└── 02-watcher-tests       └── 04-auto-start-tests
         │                           │
         └───────────┬───────────────┘
                     ▼
         Phase 3 (Example Apps)
         └── 05-example-apps-testing
```

Phase 1 and Phase 2 are independent and can be worked in parallel.
Phase 3 depends on both Phase 1 and Phase 2.

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Custom watcher paths from `config.toml` are passed through to the file watcher
- [ ] Custom extensions from `config.toml` are passed through to the file watcher
- [ ] Relative paths including `../../` are canonicalized before watching
- [ ] Absolute paths are handled correctly (not double-joined)
- [ ] Unit tests cover all path resolution scenarios
- [ ] No regressions: default `["lib"]` behavior works when no custom paths configured
- [ ] `cargo test --workspace` passes

### Phase 2 Complete When:
- [ ] `auto_start = true` in `launch.toml` causes fdemon to auto-launch on startup
- [ ] `behavior.auto_start = true` in `config.toml` triggers auto-launch
- [ ] Auto-launch failure falls back to showing the dialog
- [ ] Without any auto_start, the dialog is shown as before
- [ ] Existing broken-behavior test is updated
- [ ] Unit tests cover all auto-start scenarios
- [ ] `cargo test --workspace` passes

### Phase 3 Complete When:
- [ ] Example apps exercise all watcher path and auto_start edge cases
- [ ] Manual test procedures are documented
- [ ] All scenarios can be reproduced with `cargo run -- example/appN`

---

## Milestone Deliverable

Both configuration bugs are fixed, comprehensively tested with unit tests, and backed by example apps that allow manual verification of the fixes across various edge cases.
