## Task: Fix Watcher Path Pass-through

**Objective**: Wire `settings.watcher.paths` and `settings.watcher.extensions` from config.toml into the `WatcherConfig` builder in `Engine::start_file_watcher`, and add path canonicalization in the watcher's `run_watcher` loop.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/engine.rs:748-753`: Add `.with_paths()` and `.with_extensions()` to `WatcherConfig` builder
- `crates/fdemon-app/src/watcher/mod.rs:217-228`: Canonicalize paths before passing to `debouncer.watch()`

### Details

**Change 1: engine.rs — Pass settings through to WatcherConfig**

In `start_file_watcher()`, the `WatcherConfig::new()` call currently only passes `debounce_ms` and `auto_reload`. Add the missing paths and extensions:

```rust
fn start_file_watcher(
    project_path: &Path,
    settings: &Settings,
    msg_tx: mpsc::Sender<Message>,
) -> Option<FileWatcher> {
    let mut watcher = FileWatcher::new(
        project_path.to_path_buf(),
        WatcherConfig::new()
            .with_paths(settings.watcher.paths.iter().map(PathBuf::from).collect())
            .with_extensions(settings.watcher.extensions.clone())
            .with_debounce_ms(settings.watcher.debounce_ms)
            .with_auto_reload(settings.watcher.auto_reload),
    );
    // ... rest unchanged
}
```

**Change 2: watcher/mod.rs — Canonicalize paths before watching**

In `run_watcher()`, the loop that adds watch paths should canonicalize after joining:

```rust
for relative_path in &config.paths {
    let full_path = if relative_path.is_absolute() {
        relative_path.clone()
    } else {
        project_root.join(relative_path)
    };
    // Canonicalize to resolve ../.. and symlinks
    let canonical = full_path.canonicalize().unwrap_or(full_path);
    if canonical.exists() {
        if let Err(e) = debouncer.watch(&canonical, RecursiveMode::Recursive) {
            warn!("Failed to watch {}: {}", canonical.display(), e);
        } else {
            info!("Watching: {}", canonical.display());
        }
    } else {
        warn!("Watch path does not exist: {}", canonical.display());
    }
}
```

Note: `config.paths` is `Vec<PathBuf>`, so `relative_path` already has `is_absolute()` available. The `canonicalize()` call resolves `../../` components and follows symlinks. The `unwrap_or(full_path)` fallback preserves behavior for paths that don't exist yet (which already emit a warning).

### Acceptance Criteria

1. `settings.watcher.paths` from `config.toml` are used instead of the hardcoded `["lib"]` default
2. `settings.watcher.extensions` from `config.toml` are used instead of the hardcoded `["dart"]` default
3. Relative paths like `../../` are correctly canonicalized before passing to `notify`
4. Absolute paths are not double-joined with `project_root`
5. Default behavior is preserved when `config.toml` has no `[watcher]` section (defaults: `paths = ["lib"]`, `extensions = ["dart"]`)
6. Non-existent paths still produce a warning log

### Testing

Run existing watcher tests to verify no regressions:

```bash
cargo test -p fdemon-app -- watcher
cargo test -p fdemon-app -- engine
```

### Notes

- `WatcherConfig::with_paths()` and `WatcherConfig::with_extensions()` already exist (`watcher/mod.rs:65-80`) — this is purely a wiring fix
- The `WatcherSettings` struct in `config/types.rs` already correctly parses `paths` and `extensions` from TOML
- `canonicalize()` requires the path to exist at call time; for newly-created directories, the fallback is the raw path (which will trigger the "does not exist" warning)

---

## Completion Summary

**Status:** Not Started
