## Task: Daemon-Layer Stdout Readiness Signaling

**Objective**: Extend the daemon-layer `CustomLogCapture` to optionally match a regex pattern against stdout lines and signal readiness via a `oneshot` channel, enabling the `stdout` ready check type.

**Depends on**: None

### Scope

- `crates/fdemon-daemon/src/native_logs/custom.rs`: Add `ready_pattern` and `ready_tx` to daemon config, integrate pattern matching into `run_custom_capture()`

### Details

#### 1. Extend Daemon `CustomSourceConfig`

Add two optional fields to the daemon-layer struct at `custom.rs:36`:

```rust
#[derive(Clone)]
pub struct CustomSourceConfig {
    // ... existing fields ...

    /// Optional regex pattern to match against stdout lines for readiness signaling.
    /// When set, each stdout line is checked against this pattern. On first match,
    /// the `ready_tx` sender is fired and pattern matching stops.
    pub ready_pattern: Option<String>,
}
```

The `ready_tx` is **not** on the config struct — it's passed as a separate parameter because `oneshot::Sender` is not `Clone` and cannot live on a `Clone` struct. Instead, it's passed to `run_custom_capture()` directly.

#### 2. Update `CustomLogCapture::spawn()` Signature

Add an optional `ready_tx` parameter. Since the `NativeLogCapture` trait's `spawn()` doesn't accept extra args, we add a separate method:

```rust
impl CustomLogCapture {
    /// Spawn with an optional readiness signal sender.
    ///
    /// If `ready_tx` is provided alongside a `ready_pattern` in the config,
    /// the capture loop will fire `ready_tx` when the pattern first matches
    /// a stdout line.
    pub fn spawn_with_readiness(
        &self,
        ready_tx: Option<tokio::sync::oneshot::Sender<()>>,
    ) -> Option<NativeLogHandle> {
        let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(EVENT_CHANNEL_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let config = self.config.clone();

        let task_handle = tokio::spawn(async move {
            run_custom_capture(config, event_tx, shutdown_rx, ready_tx).await;
        });

        Some(NativeLogHandle {
            event_rx,
            shutdown_tx,
            task_handle,
        })
    }
}
```

Keep the existing `NativeLogCapture::spawn()` impl working by delegating:

```rust
impl NativeLogCapture for CustomLogCapture {
    fn spawn(&self) -> Option<NativeLogHandle> {
        self.spawn_with_readiness(None)
    }
}
```

#### 3. Modify `run_custom_capture()` for Pattern Matching

Add `ready_tx` parameter and pattern matching logic:

```rust
async fn run_custom_capture(
    config: CustomSourceConfig,
    event_tx: mpsc::Sender<NativeLogEvent>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut ready_tx: Option<tokio::sync::oneshot::Sender<()>>,
) {
    // Compile ready pattern if provided
    let mut ready_regex = config.ready_pattern.as_ref().and_then(|p| {
        match regex::Regex::new(p) {
            Ok(r) => Some(r),
            Err(e) => {
                tracing::warn!(
                    "Custom source '{}': invalid ready pattern '{}': {}",
                    config.name, p, e
                );
                // Drop the sender — receiver will get RecvError (treated as failure)
                drop(ready_tx.take());
                None
            }
        }
    });

    // ... existing spawn + stdout setup code ...

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    // ... existing shutdown logic ...
                    break;
                }
            }

            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        // Check stdout readiness pattern BEFORE log parsing
                        if let (Some(re), Some(_)) = (&ready_regex, &ready_tx) {
                            if re.is_match(&line) {
                                if let Some(tx) = ready_tx.take() {
                                    let _ = tx.send(());
                                }
                                // Clear regex — no further matching needed
                                ready_regex = None;
                                tracing::debug!(
                                    "Custom source '{}': stdout ready pattern matched",
                                    config.name
                                );
                            }
                        }

                        // ... existing parse_line + tag filtering + event_tx.send ...
                    }
                    // ... existing EOF + Err handling ...
                }
            }
        }
    }

    // If process exits before pattern matched, dropping ready_tx signals failure
    // (the oneshot::Receiver gets a RecvError)
}
```

Key design points:
- Pattern check happens **before** log parsing (same line, same loop iteration) — no stream splitting
- `ready_tx.take()` ensures the signal fires exactly once
- `ready_regex = None` after match stops all further regex evaluation (zero cost after match)
- If the process exits before matching, `ready_tx` is dropped implicitly, causing the receiver to get `RecvError` — the app layer interprets this as "ready check failed"

#### 4. Update `create_custom_log_capture()`

No change needed — the factory returns a `CustomLogCapture` via the trait. The `spawn_with_readiness()` method is called directly when the caller needs readiness signaling (the app-layer `spawn_pre_app_sources()` in Task 06).

#### 5. Update Test Helper

Update `make_config()` in the test module to include the new field:

```rust
fn make_config(command: &str, args: Vec<&str>, format: OutputFormat) -> CustomSourceConfig {
    CustomSourceConfig {
        // ... existing fields ...
        ready_pattern: None,
    }
}
```

Also update all test sites that construct `CustomSourceConfig` directly (there are ~6 in the test module).

### Acceptance Criteria

1. Existing `spawn()` (via `NativeLogCapture` trait) still works identically — no behavioral change for callers that don't use readiness
2. `spawn_with_readiness(None)` behaves identically to `spawn()`
3. `spawn_with_readiness(Some(tx))` with `ready_pattern = Some("Ready")` fires `tx` when a stdout line matches
4. Pattern matching stops after first match (subsequent matching lines don't cause issues)
5. If process exits before pattern matches, `ready_tx` is dropped (receiver gets `RecvError`)
6. Invalid regex in `ready_pattern` logs a warning and drops `ready_tx` immediately
7. Log events continue flowing normally during and after pattern matching
8. All existing tests pass without modification (only `make_config` and direct struct constructors need the new field)
9. `cargo test -p fdemon-daemon` passes

### Testing

```rust
#[tokio::test]
async fn test_stdout_ready_pattern_fires_on_match() {
    let config = CustomSourceConfig {
        name: "ready-test".to_string(),
        command: "printf".to_string(),
        args: vec!["starting\\nServer ready on port 8080\\nhandling requests\\n".to_string()],
        format: OutputFormat::Raw,
        working_dir: None,
        env: HashMap::new(),
        exclude_tags: vec![],
        include_tags: vec![],
        ready_pattern: Some("Server ready".to_string()),
    };

    let capture = CustomLogCapture::new(config);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let _handle = capture.spawn_with_readiness(Some(ready_tx));

    // ready_rx should resolve (not timeout)
    let result = timeout(Duration::from_secs(2), ready_rx).await;
    assert!(result.is_ok(), "ready signal should fire on pattern match");
}

#[tokio::test]
async fn test_stdout_ready_pattern_no_match_drops_tx() {
    let config = CustomSourceConfig {
        name: "no-match-test".to_string(),
        command: "echo".to_string(),
        args: vec!["no match here".to_string()],
        format: OutputFormat::Raw,
        working_dir: None,
        env: HashMap::new(),
        exclude_tags: vec![],
        include_tags: vec![],
        ready_pattern: Some("will not match".to_string()),
    };

    let capture = CustomLogCapture::new(config);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let _handle = capture.spawn_with_readiness(Some(ready_tx));

    // Process exits quickly without matching — ready_rx should error
    let result = timeout(Duration::from_secs(2), ready_rx).await;
    assert!(
        matches!(result, Ok(Err(_))),
        "ready_rx should get RecvError when process exits without matching"
    );
}

#[tokio::test]
async fn test_stdout_ready_pattern_none_no_signal() {
    // No ready_pattern set, ready_tx provided — tx should be dropped (not fired)
    let config = CustomSourceConfig {
        name: "no-pattern".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        format: OutputFormat::Raw,
        working_dir: None,
        env: HashMap::new(),
        exclude_tags: vec![],
        include_tags: vec![],
        ready_pattern: None,
    };

    let capture = CustomLogCapture::new(config);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let _handle = capture.spawn_with_readiness(Some(ready_tx));

    // No pattern to match — ready_tx dropped when process exits
    let result = timeout(Duration::from_secs(2), ready_rx).await;
    assert!(matches!(result, Ok(Err(_))));
}

#[tokio::test]
async fn test_stdout_ready_logs_still_flow_after_match() {
    let config = CustomSourceConfig {
        name: "logs-after-ready".to_string(),
        command: "printf".to_string(),
        args: vec!["ready\\nlog after ready\\n".to_string()],
        format: OutputFormat::Raw,
        working_dir: None,
        env: HashMap::new(),
        exclude_tags: vec![],
        include_tags: vec![],
        ready_pattern: Some("ready".to_string()),
    };

    let capture = CustomLogCapture::new(config);
    let (ready_tx, _ready_rx) = tokio::sync::oneshot::channel();
    let handle = capture.spawn_with_readiness(Some(ready_tx)).unwrap();

    let mut event_rx = handle.event_rx;
    let mut events = Vec::new();
    loop {
        match timeout(Duration::from_millis(500), event_rx.recv()).await {
            Ok(Some(event)) => events.push(event),
            _ => break,
        }
    }

    // Both lines should appear as log events (pattern matching doesn't consume lines)
    assert!(events.len() >= 2, "both lines should be forwarded as log events");
}
```

### Notes

- The `regex` crate needs to be in `fdemon-daemon/Cargo.toml`. Check if it's already a dependency — it may be transitively available but should be listed explicitly for the `Regex::new()` call.
- The `tokio::sync::oneshot` is already available since `fdemon-daemon` depends on `tokio`.
- The pattern check happens on the **raw** line before `parse_line()` — this is intentional so the pattern can match regardless of output format (the user writes the pattern to match their process's actual stdout, not the parsed log message).
- This task is deliberately scoped to the daemon layer only. The app-layer integration (constructing `ready_tx`, awaiting `ready_rx`) is handled in Task 04 (ready check execution) and Task 06 (spawn_pre_app_sources action).

---

## Completion Summary

**Status:** Not Started
