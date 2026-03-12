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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `example/app1/.fdemon/launch.toml` | Added `auto_start = true` to the "App1 | Default" config |
| `example/app1/.fdemon/config.toml` | Updated `[watcher]` paths to `["lib", "../app2/lib"]` for cross-project test |
| `example/app3/.fdemon/launch.toml` | Created: 3 configs, "Staging" has `auto_start = true` (Issue #18 exact reproduction) |
| `example/app3/.fdemon/config.toml` | Created: `behavior.auto_start = false`, `native_logs.enabled = false` |
| `example/app3/pubspec.yaml` | Created: minimal Flutter project descriptor |
| `example/app3/lib/main.dart` | Created: minimal Flutter app with descriptive comment |
| `example/app4/.fdemon/config.toml` | Created: `paths = ["lib", "../../shared_lib", "../app1/lib"]`, `extensions = ["dart", "json"]` |
| `example/app4/.fdemon/launch.toml` | Created: single config, no auto_start |
| `example/app4/pubspec.yaml` | Created: minimal Flutter project descriptor |
| `example/app4/lib/main.dart` | Created: minimal Flutter app with descriptive comment |
| `example/shared_lib/shared_utils.dart` | Created: sample Dart file for cross-project watcher testing |
| `example/TESTING.md` | Created: manual test procedures for Tests A-H |

### Notable Decisions/Tradeoffs

1. **app1 `behavior.auto_start` left as-is**: The existing `config.toml` already has `[behavior] auto_start = true`. The task asked to add `auto_start = true` to the first launch config entry (not the behavior section), which was done. Both the global behavior flag and the per-config flag are now present, exercising the per-config path.

2. **Minimal Flutter app structure for app3/app4**: Per the task notes, app3 and app4 only need `pubspec.yaml` and `lib/main.dart` for project discovery. They do not have the full Flutter scaffold (no `android/`, `ios/`, etc.) since they are fixtures, not runnable apps. The task explicitly stated "they don't need to be runnable Flutter apps, just recognized as Flutter projects."

3. **`shared_lib` is not a Flutter project**: Intentionally has no `pubspec.yaml` — it is a raw shared code directory, matching the monorepo pattern described in the task.

4. **Pre-existing snapshot test failures**: 4 `fdemon-tui` snapshot tests fail due to a version string mismatch (`v0.1.0` vs `v0.2.1`). Confirmed pre-existing before this task's changes.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - 826 passed, 4 failed (pre-existing snapshot failures unrelated to this task)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **app3/app4 not runnable**: Without a full Flutter scaffold, `cargo run -- example/app3` will start fdemon but flutter will fail to build. The fixture is only suitable for testing fdemon startup behaviour (dialog vs auto-launch) and watcher path resolution, not end-to-end hot reload.

2. **`../../shared_lib` path traversal**: The resolved path `example/shared_lib` exists, but if fdemon is invoked from a non-standard working directory the relative resolution may differ. The watcher path fix (task 01) should handle canonicalization relative to the project root.
