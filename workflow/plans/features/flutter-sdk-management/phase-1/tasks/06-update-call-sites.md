## Task: Update Daemon Call Sites

**Objective**: Modify `process.rs`, `devices.rs`, and `emulators.rs` to accept a `FlutterExecutable` (or `FlutterSdk`) instead of hardcoding `Command::new("flutter")`. This makes the daemon layer SDK-aware without changing its responsibilities.

**Depends on**: 01-core-types

### Scope

- `crates/fdemon-daemon/src/process.rs`: Update `spawn_internal()` and public spawn methods
- `crates/fdemon-daemon/src/devices.rs`: Update `run_flutter_devices()` and public discovery functions
- `crates/fdemon-daemon/src/emulators.rs`: Update `run_flutter_emulators()` and `run_flutter_emulator_launch()`
- `crates/fdemon-daemon/src/lib.rs`: Update public re-exports if signatures change

### Details

#### Current State (4 hardcoded call sites)

| File | Line | Current Code |
|------|------|-------------|
| `process.rs` | 64 | `Command::new("flutter").args(args).current_dir(project_path)` |
| `devices.rs` | 180 | `Command::new("flutter").args(["devices", "--machine"])` |
| `emulators.rs` | 125 | `Command::new("flutter").args(["emulators", "--machine"])` |
| `emulators.rs` | 291 | `Command::new("flutter").args(["emulators", "--launch", id])` |

#### Strategy: Accept `&FlutterExecutable` parameter

Rather than threading the entire `FlutterSdk` through the daemon, only pass `&FlutterExecutable` — the daemon only needs to know how to invoke the binary, not where it came from or what version it is.

#### 1. Update `process.rs` — `spawn_internal()`

**Before:**
```rust
fn spawn_internal(
    args: &[String],
    project_path: &Path,
    event_tx: mpsc::Sender<DaemonEvent>,
) -> Result<Self> {
    let child = Command::new("flutter")
        .args(args)
        .current_dir(project_path)
        // ...
        .spawn()?;
}
```

**After:**
```rust
fn spawn_internal(
    flutter: &FlutterExecutable,
    args: &[String],
    project_path: &Path,
    event_tx: mpsc::Sender<DaemonEvent>,
) -> Result<Self> {
    let mut cmd = flutter.command();
    let child = cmd
        .args(args)
        .current_dir(project_path)
        // ...
        .spawn()?;
}
```

Update all three public methods that delegate to `spawn_internal`:

```rust
pub async fn spawn(
    flutter: &FlutterExecutable,
    project_path: &Path,
    event_tx: mpsc::Sender<DaemonEvent>,
) -> Result<Self>

pub async fn spawn_with_device(
    flutter: &FlutterExecutable,
    project_path: &Path,
    device_id: &str,
    event_tx: mpsc::Sender<DaemonEvent>,
) -> Result<Self>

pub async fn spawn_with_args(
    flutter: &FlutterExecutable,
    project_path: &Path,
    args: Vec<String>,
    event_tx: mpsc::Sender<DaemonEvent>,
) -> Result<Self>
```

**Error mapping**: Keep the existing `io::ErrorKind::NotFound → Error::FlutterNotFound` mapping at line 73-75. This still makes sense — if the executable path doesn't exist at runtime, the error is the same.

#### 2. Update `devices.rs` — `run_flutter_devices()`

**Before:**
```rust
async fn run_flutter_devices() -> Result<FlutterOutput> {
    let output = Command::new("flutter")
        .args(["devices", "--machine"])
        // ...
        .output().await?;
}
```

**After:**
```rust
async fn run_flutter_devices(flutter: &FlutterExecutable) -> Result<FlutterOutput> {
    let mut cmd = flutter.command();
    let output = cmd
        .args(["devices", "--machine"])
        // ...
        .output().await?;
}
```

Update the public functions:

```rust
pub async fn discover_devices(flutter: &FlutterExecutable) -> Result<DeviceDiscoveryResult>

pub async fn discover_devices_with_timeout(
    flutter: &FlutterExecutable,
    timeout_duration: Duration,
) -> Result<DeviceDiscoveryResult>
```

#### 3. Update `emulators.rs` — Both flutter invocations

**Discovery:**
```rust
async fn run_flutter_emulators(flutter: &FlutterExecutable) -> Result<FlutterOutput> {
    let mut cmd = flutter.command();
    let output = cmd
        .args(["emulators", "--machine"])
        // ...
        .output().await?;
}
```

**Launch:**
```rust
async fn run_flutter_emulator_launch(
    flutter: &FlutterExecutable,
    emulator_id: &str,
    cold_boot: bool,
) -> Result<FlutterOutput> {
    let mut cmd = flutter.command();
    cmd.args(["emulators", "--launch", emulator_id]);
    if cold_boot {
        cmd.arg("--cold");
    }
    // ...
}
```

Update public functions:

```rust
pub async fn discover_emulators(flutter: &FlutterExecutable) -> Result<EmulatorDiscoveryResult>

pub async fn discover_emulators_with_timeout(
    flutter: &FlutterExecutable,
    timeout_duration: Duration,
) -> Result<EmulatorDiscoveryResult>

pub async fn launch_emulator(
    flutter: &FlutterExecutable,
    emulator_id: &str,
) -> Result<EmulatorLaunchResult>

pub async fn launch_emulator_cold(
    flutter: &FlutterExecutable,
    emulator_id: &str,
) -> Result<EmulatorLaunchResult>

pub async fn launch_emulator_with_options(
    flutter: &FlutterExecutable,
    emulator_id: &str,
    options: EmulatorLaunchOptions,
    timeout_duration: Duration,
) -> Result<EmulatorLaunchResult>
```

#### 4. Update `lib.rs` re-exports

If any re-exported function signatures changed, update `lib.rs`. The existing re-exports are:
```rust
pub use devices::{discover_devices, discover_devices_with_timeout, ...};
pub use emulators::{discover_emulators, launch_emulator, ...};
pub use process::FlutterProcess;
```

These re-exports don't need to change — they're just paths. But callers in `fdemon-app` will need to update their call sites (handled in task 07).

#### 5. Handle `WindowsBatch` variant

The `FlutterExecutable::command()` method (from task 01) handles this transparently:

```rust
impl FlutterExecutable {
    pub fn command(&self) -> Command {
        match self {
            Self::Direct(path) => Command::new(path),
            Self::WindowsBatch(path) => {
                let mut cmd = Command::new("cmd");
                cmd.args(["/c", &path.to_string_lossy()]);
                cmd
            }
        }
    }
}
```

This means the call sites don't need any Windows-specific logic — they just call `flutter.command()` and it works.

### Acceptance Criteria

1. `spawn_internal()` accepts `&FlutterExecutable` as first parameter
2. All three public `FlutterProcess::spawn*` methods accept `&FlutterExecutable`
3. `discover_devices()` and `discover_devices_with_timeout()` accept `&FlutterExecutable`
4. All public emulator functions accept `&FlutterExecutable`
5. No more `Command::new("flutter")` anywhere in the daemon crate
6. `FlutterNotFound` error mapping is preserved for `io::ErrorKind::NotFound`
7. `cargo check -p fdemon-daemon` compiles
8. Existing tests are updated to pass `FlutterExecutable::Direct` with a mock path
9. The `WindowsBatch` path is handled transparently via `FlutterExecutable::command()`

### Testing

Existing tests in `process.rs`, `devices.rs`, and `emulators.rs` need to be updated to supply a `&FlutterExecutable` argument. Since existing tests don't actually spawn a real Flutter process (they use mock outputs or test the parsing logic), the change is mechanical:

```rust
// Before (in tests):
let result = discover_devices().await;

// After:
let flutter = FlutterExecutable::Direct(PathBuf::from("flutter"));
let result = discover_devices(&flutter).await;
```

**New tests for WindowsBatch:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flutter_executable_direct_command() {
        let exe = FlutterExecutable::Direct(PathBuf::from("/usr/local/flutter/bin/flutter"));
        let cmd = exe.command();
        // Verify command is created with the direct path
        // (Command internals aren't easily inspectable, but we can verify it doesn't panic)
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_flutter_executable_windows_batch_command() {
        let exe = FlutterExecutable::WindowsBatch(PathBuf::from(r"C:\flutter\bin\flutter.bat"));
        let cmd = exe.command();
        // On Windows, this should create cmd /c <path>
    }
}
```

### Notes

- **Breaking API change for fdemon-daemon consumers**: All public spawn/discover/launch functions now require a `&FlutterExecutable` parameter. The only consumer is `fdemon-app` (specifically the action dispatcher and engine), which is updated in task 07.
- **Test updates are mechanical**: Most existing daemon tests test parsing logic, not process spawning. They construct mock outputs and call parsing functions. Only tests that call the public spawn/discover functions need the `FlutterExecutable` parameter added.
- **No functional change yet**: After this task, the daemon API requires a `FlutterExecutable`, but nothing resolves one yet. Task 07 wires the full pipeline. During development, tests can use `FlutterExecutable::Direct(PathBuf::from("flutter"))` as a placeholder that behaves identically to the old `Command::new("flutter")`.
- **Consider a temporary backward-compat helper**: If the compile-and-test cycle is painful during development, a private `fn default_flutter() -> FlutterExecutable` can serve as a bridge. Remove it in task 07.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/flutter_sdk/mod.rs` | Created new module with `FlutterExecutable` enum and `command()`/`path()` methods (since task 01 was in a different worktree) |
| `crates/fdemon-daemon/src/lib.rs` | Added `pub mod flutter_sdk;` and `pub use flutter_sdk::FlutterExecutable;` re-export |
| `crates/fdemon-daemon/src/process.rs` | Updated `spawn_internal()` and all three public `spawn*` methods to accept `&FlutterExecutable` as first parameter; moved `Command` import into `#[cfg(test)]` module |
| `crates/fdemon-daemon/src/devices.rs` | Updated `run_flutter_devices()`, `discover_devices()`, and `discover_devices_with_timeout()` to accept `&FlutterExecutable`; updated integration test |
| `crates/fdemon-daemon/src/emulators.rs` | Updated `run_flutter_emulators()`, `run_flutter_emulator_launch()`, `discover_emulators()`, `discover_emulators_with_timeout()`, `launch_emulator()`, `launch_emulator_cold()`, and `launch_emulator_with_options()` to accept `&FlutterExecutable`; updated integration tests |

### Notable Decisions/Tradeoffs

1. **Created `flutter_sdk` in daemon crate**: Task 01 was completed in a different worktree (agent-a8e74dad) that has not been merged yet. This worktree needed `FlutterExecutable` to exist, so the module was created here. The implementation matches task 01's output exactly.

2. **Simplified `flutter_sdk/mod.rs`**: The current worktree's `fdemon-core` does not have the `FlutterSdkInvalid` error variant needed by `validate_sdk_path()` (that was also part of task 01). Since task 06 only requires `FlutterExecutable`, the module only includes the type definition and `command()`/`path()` methods, without `validate_sdk_path()` or `read_version_file()`. These can be added when the worktrees are merged.

3. **Preserved `FlutterNotFound` error mapping**: The `io::ErrorKind::NotFound` to `Error::FlutterNotFound` mapping is preserved in all three call sites as required.

4. **`launch_ios_simulator` unchanged**: This function uses `Command::new("open")` (not Flutter) so it correctly keeps the `tokio::process::Command` import in `emulators.rs`.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon` - Passed (583 tests, 3 ignored)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed
- `cargo fmt -p fdemon-daemon -- --check` - Passed

### Risks/Limitations

1. **Worktree merge conflict**: When the task-01 worktree (agent-a8e74dad) is merged, there will be a conflict in `flutter_sdk/mod.rs`. The merged version should be a superset of what is here, adding `validate_sdk_path()`, `read_version_file()`, `FlutterSdk`, `SdkSource`, and the corresponding `fdemon-core` error types.

2. **fdemon-app will not compile**: As expected and documented in the task, `fdemon-app` calls the old signatures without `&FlutterExecutable`. This is intentional — task 07 will wire the app layer.
