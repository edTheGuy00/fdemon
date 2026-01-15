# Task: Config Writer Improvements (File Locking & Race Condition)

## Summary

Address the minor issues in the config writer: add file locking for concurrent write protection and fix the potential race condition in ConfigAutoSaver.

## Files

| File | Action |
|------|--------|
| `src/config/writer.rs` | Modify (add locking, fix race) |
| `Cargo.toml` | Modify (add fs2 dependency) |

## Background

The code review identified two minor issues:
1. **No file locking**: Concurrent writes to `.fdemon/launch.toml` are not protected
2. **Race condition**: Multiple rapid saves in ConfigAutoSaver could lose intermediate state

## Implementation

### 1. Add fs2 dependency for file locking

```toml
# Cargo.toml
[dependencies]
fs2 = "0.4"  # Cross-platform file locking
```

### 2. Add file locking to save_fdemon_configs

Location: `src/config/writer.rs:42`

```rust
use fs2::FileExt;
use std::fs::OpenOptions;

pub fn save_fdemon_configs(project_path: &Path, configs: &LaunchConfigs) -> Result<(), Error> {
    let config_dir = project_path.join(".fdemon");
    std::fs::create_dir_all(&config_dir)?;

    let config_path = config_dir.join("launch.toml");
    let content = serialize_configs(configs)?;

    // Open file with exclusive lock
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&config_path)?;

    // Acquire exclusive lock (blocks if another process has lock)
    file.lock_exclusive()?;

    // Write content
    use std::io::Write;
    let mut file = file;
    file.write_all(content.as_bytes())?;
    file.flush()?;

    // Lock is automatically released when file is dropped
    Ok(())
}
```

### 3. Fix ConfigAutoSaver race condition

Location: `src/config/writer.rs:222-242`

The issue is that multiple rapid saves could overlap. Options:

**Option A: Use a write queue with latest-wins**

```rust
use tokio::sync::mpsc;

pub struct ConfigAutoSaver {
    tx: mpsc::Sender<SaveRequest>,
}

struct SaveRequest {
    project_path: PathBuf,
    configs: LaunchConfigs,
}

impl ConfigAutoSaver {
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::channel::<SaveRequest>(10);

        tokio::spawn(async move {
            // Process only the latest request after debounce
            let mut pending: Option<SaveRequest> = None;
            let mut debounce = tokio::time::interval(Duration::from_millis(500));

            loop {
                tokio::select! {
                    Some(req) = rx.recv() => {
                        // Always keep the latest request
                        pending = Some(req);
                    }
                    _ = debounce.tick() => {
                        if let Some(req) = pending.take() {
                            if let Err(e) = save_fdemon_configs(&req.project_path, &req.configs) {
                                tracing::error!("Auto-save failed: {}", e);
                            }
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    pub fn schedule_save(&self, project_path: PathBuf, configs: LaunchConfigs) {
        let _ = self.tx.try_send(SaveRequest { project_path, configs });
    }
}
```

**Option B: Use AtomicBool to skip overlapping saves**

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct ConfigAutoSaver {
    saving: Arc<AtomicBool>,
}

impl ConfigAutoSaver {
    pub fn schedule_save(&self, project_path: PathBuf, configs: LaunchConfigs) {
        let saving = self.saving.clone();

        // Skip if already saving
        if saving.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            tracing::debug!("Skipping save - already in progress");
            return;
        }

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;

            if let Err(e) = save_fdemon_configs(&project_path, &configs) {
                tracing::error!("Auto-save failed: {}", e);
            }

            saving.store(false, Ordering::SeqCst);
        });
    }
}
```

## Acceptance Criteria

1. File locking added using fs2 crate
2. Concurrent writes are safely serialized
3. Rapid saves don't lose intermediate state
4. All existing tests pass
5. `cargo test writer` passes

## Verification

```bash
cargo fmt && cargo check && cargo test writer && cargo clippy -- -D warnings
```

## Manual Testing

1. Open two terminals
2. In both, trigger config saves rapidly
3. Verify no file corruption or lost data
4. Check logs for any locking errors

## Notes

- fs2 provides cross-platform file locking (works on Windows, macOS, Linux)
- Advisory locks don't prevent other processes from writing (they must also use locking)
- Option A (write queue) is more robust but more complex
- Option B (skip overlapping) is simpler but may drop intermediate saves
