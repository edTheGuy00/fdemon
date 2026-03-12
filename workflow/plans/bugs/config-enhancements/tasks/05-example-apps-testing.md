## Task: Example Apps and Manual Testing

**Objective**: Create example app configurations in `/Users/ed/Dev/zabin/flutter-demon/example/` that exercise all watcher path and auto_start edge cases, and document manual test procedures.

**Depends on**: 01-fix-watcher-paths, 02-watcher-tests, 03-fix-auto-start, 04-auto-start-tests

### Scope

- `example/app1/.fdemon/config.toml`: Update with relative watcher paths
- `example/app1/.fdemon/launch.toml`: Add `auto_start = true` to one config
- `example/app3/` — **NEW**: Multi-config auto_start test fixture
- `example/app4/` — **NEW**: Watcher path edge cases test fixture
- `example/shared_lib/` — **NEW**: Shared directory for cross-project watcher testing
- `example/TESTING.md` — **NEW**: Manual test procedures

### Details

**1. Update app1 — Add auto_start and custom watcher paths**

`example/app1/.fdemon/launch.toml`:
```toml
[[configurations]]
name = "App1 | Default"
device = "auto"
auto_start = true    # ← ADD: should trigger auto-launch

[[configurations]]
name = "App1 | Staging (with env file)"
device = "auto"
extra_args = [
    "--dart-define-from-file=envs/staging.env.json"
]
```

`example/app1/.fdemon/config.toml` — update `[watcher]` section:
```toml
[watcher]
paths = ["lib", "../app2/lib"]   # Watch own lib + app2's lib
debounce_ms = 500
auto_reload = true
extensions = ["dart"]
```

**2. Create app3 — Multi-config auto_start scenario (Issue #18 exact reproduction)**

```
example/app3/
├── .fdemon/
│   ├── config.toml    # behavior.auto_start = false
│   └── launch.toml    # 3 configs, middle one has auto_start = true
├── lib/
│   └── main.dart      # Minimal Flutter app
└── pubspec.yaml
```

`example/app3/.fdemon/launch.toml`:
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

**3. Create app4 — Watcher path edge cases (Issue #17 reproduction)**

```
example/app4/
├── .fdemon/
│   ├── config.toml    # Various watcher path scenarios
│   └── launch.toml    # Simple single config
├── lib/
│   └── main.dart
└── pubspec.yaml
```

`example/app4/.fdemon/config.toml`:
```toml
[watcher]
paths = ["lib", "../../shared_lib", "../app1/lib"]
debounce_ms = 500
auto_reload = true
extensions = ["dart", "json"]    # Also watch .json files
```

**4. Create shared_lib directory**

```
example/shared_lib/
└── shared_utils.dart    # Sample Dart file for cross-project watcher testing
```

This directory is what `../../shared_lib` from `app4` should resolve to.

**5. Document manual test procedures in TESTING.md**

Create `example/TESTING.md` with step-by-step procedures:

- **Test A**: Verify default watcher paths (app2 — baseline)
- **Test B**: Verify cross-project watcher paths (app1 — `../app2/lib`)
- **Test C**: Verify `../../` watcher paths (app4 — `../../shared_lib`)
- **Test D**: Verify auto_start with single config (app1)
- **Test E**: Verify auto_start with multiple configs (app3)
- **Test F**: Verify no auto_start shows dialog (app2)
- **Test G**: Verify non-existent watcher path warning (add bad path to app4)
- **Test H**: Verify custom extensions (app4 — `.json` files)

### Acceptance Criteria

1. `cargo run -- example/app1` starts and auto-launches (auto_start config present)
2. `cargo run -- example/app2` starts and shows NewSessionDialog (no auto_start)
3. `cargo run -- example/app3` starts and auto-launches with "Staging" config
4. `cargo run -- example/app4` watches `lib/`, `../../shared_lib/`, and `../app1/lib/`
5. Editing a file in `example/shared_lib/` triggers hot reload when running app4
6. `example/TESTING.md` documents all test procedures clearly

### Testing

Manual testing with each example app. Verify by:
1. Running `cargo run -- example/appN`
2. Checking log output for "Watching: ..." messages (watcher paths)
3. Observing startup behavior (dialog vs auto-launch)
4. Editing files in watched directories and verifying hot reload triggers

### Notes

- app3 and app4 need minimal `pubspec.yaml` and `lib/main.dart` to be recognized as Flutter projects
- The `shared_lib` directory is intentionally outside any Flutter project — it's a shared code directory pattern common in monorepos
- Keep the new Flutter apps minimal — they don't need to be runnable Flutter apps, just recognized as Flutter projects by fdemon's project discovery

---

## Completion Summary

**Status:** Not Started
