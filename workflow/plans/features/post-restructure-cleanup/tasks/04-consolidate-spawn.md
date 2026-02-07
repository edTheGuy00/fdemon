## Task: Consolidate FlutterProcess spawn methods

**Objective**: Refactor 3 nearly-identical spawn methods in `FlutterProcess` into a single `spawn_internal()` with thin public wrappers, eliminating ~70 lines of duplication.

**Review Issue**: #6 (MINOR) - FlutterProcess spawn method duplication

**Depends on**: None

### Scope

- `crates/fdemon-daemon/src/process.rs`: Refactor lines 30-212

### Details

#### Current State

Three public methods share ~90% identical code:

| Method | Lines | Unique Part |
|--------|-------|-------------|
| `spawn(project_path, event_tx)` | 30-83 | Args: `["run", "--machine"]` |
| `spawn_with_device(project_path, device_id, event_tx)` | 88-149 | Args: `["run", "--machine", "-d", device_id]` |
| `spawn_with_args(args, project_path, event_tx)` | 155-212 | Args: caller-provided `Vec<String>` |

Each method duplicates:
- Project validation (pubspec.yaml check) - 5 lines
- Command builder (Command::new + stdin/stdout/stderr piping) - 8 lines
- Error mapping (NotFound → FlutterNotFound) - 6 lines
- Post-spawn wiring (PID logging, stdin writer, stdout/stderr readers) - 16 lines

**Total duplication: ~70 lines** (35 lines x 2 extra copies)

#### Refactored Design

```rust
impl FlutterProcess {
    /// Internal spawn implementation. All public methods delegate here.
    fn spawn_internal(
        args: &[String],
        project_path: &Path,
        event_tx: mpsc::Sender<DaemonEvent>,
    ) -> Result<Self> {
        // 1. Validate project
        let pubspec = project_path.join("pubspec.yaml");
        if !pubspec.exists() {
            return Err(Error::NoProject { path: project_path.to_path_buf() });
        }

        info!("Spawning Flutter: flutter {}", args.join(" "));

        // 2. Build and spawn command
        let mut child = Command::new("flutter")
            .args(args)
            .current_dir(project_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Error::FlutterNotFound
                } else {
                    Error::ProcessSpawn { reason: e.to_string() }
                }
            })?;

        // 3. Wire up I/O channels
        let pid = child.id();
        info!("Flutter process started with PID: {:?}", pid);

        let stdin = child.stdin.take().expect("stdin was configured");
        let (stdin_tx, stdin_rx) = mpsc::channel::<String>(32);
        tokio::spawn(Self::stdin_writer(stdin, stdin_rx));

        let stdout = child.stdout.take().expect("stdout was configured");
        tokio::spawn(Self::stdout_reader(stdout, event_tx.clone()));

        let stderr = child.stderr.take().expect("stderr was configured");
        tokio::spawn(Self::stderr_reader(stderr, event_tx));

        Ok(Self { child, stdin_tx, pid })
    }

    /// Spawn with default args (run --machine).
    pub fn spawn(
        project_path: &Path,
        event_tx: mpsc::Sender<DaemonEvent>,
    ) -> Result<Self> {
        let args = vec!["run".to_string(), "--machine".to_string()];
        Self::spawn_internal(&args, project_path, event_tx)
    }

    /// Spawn targeting a specific device.
    pub fn spawn_with_device(
        project_path: &Path,
        device_id: &str,
        event_tx: mpsc::Sender<DaemonEvent>,
    ) -> Result<Self> {
        let args = vec![
            "run".to_string(), "--machine".to_string(),
            "-d".to_string(), device_id.to_string(),
        ];
        Self::spawn_internal(&args, project_path, event_tx)
    }

    /// Spawn with caller-provided args.
    pub fn spawn_with_args(
        args: Vec<String>,
        project_path: &Path,
        event_tx: mpsc::Sender<DaemonEvent>,
    ) -> Result<Self> {
        Self::spawn_internal(&args, project_path, event_tx)
    }
}
```

#### Public API Changes

**None.** All three public method signatures remain identical. Only the internal implementation changes. This is a pure refactoring -- all callers continue to work without modification.

### Acceptance Criteria

1. Single `spawn_internal()` method contains all shared logic
2. `spawn()`, `spawn_with_device()`, `spawn_with_args()` are thin wrappers (2-5 lines each)
3. All existing tests pass without modification
4. No public API changes (callers unchanged)
5. `cargo test -p fdemon-daemon` passes
6. `cargo clippy -p fdemon-daemon -- -D warnings` passes

### Testing

Existing tests for `FlutterProcess` cover spawn behavior. No new tests needed -- the refactoring is behavior-preserving. Run the full fdemon-daemon test suite to verify.

### Notes

- `spawn()` and `spawn_with_device()` are effectively special cases of `spawn_with_args()`, so even `spawn_internal` could be skipped in favor of having them call `spawn_with_args()` directly. However, `spawn_internal` taking `&[String]` avoids the allocation from `Vec<String>` when `spawn_with_args` already has one.
- The unified log message (`"Spawning Flutter: flutter {}"`) replaces the 3 different log messages, which is acceptable since the args already contain all context.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/crates/fdemon-daemon/src/process.rs` | Refactored three nearly-identical spawn methods into single `spawn_internal()` method with thin public wrappers. Eliminated 94 lines of duplication. |

### Notable Decisions/Tradeoffs

1. **`spawn_internal` takes `&[String]` instead of `&[&str]`**: This matches the existing API of `spawn_with_args(Vec<String>)` and allows the internal method to be called without additional allocations when the caller already has a `Vec<String>`. The thin wrappers create the `Vec<String>` on the stack before delegating.

2. **Unified log message**: The single internal log message `"Spawning Flutter: flutter {}"` replaces the three different log messages that existed before. This is acceptable since the args already contain all necessary context (device ID, etc.).

3. **Removed `async` from `spawn_internal`**: Since the method is synchronous (doesn't await anything), the `async` keyword was removed. The public wrappers remain `async` for API compatibility.

4. **Simplified stdout/stderr spawning**: Replaced separate variable bindings (`stdout_tx`, `stderr_tx`) with inline `event_tx.clone()` calls, reducing verbosity while maintaining identical behavior.

### Testing Performed

- `cargo test -p fdemon-daemon` - Passed (136 tests passed, 0 failed, 3 ignored)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (no warnings)

### Risks/Limitations

None. This is a pure refactoring with no behavioral changes. All existing tests pass without modification, confirming the public API remains unchanged.
