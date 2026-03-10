## Task: Share Channel Capacity Constant and Derive `Clone` on Config Structs

**Objective**: Extract the hardcoded channel capacity `256` into a shared constant, and derive `Clone` on `AndroidLogConfig` / `MacOsLogConfig` to replace manual field-by-field cloning.

**Depends on**: None

**Review Issues:** #9 (Minor), #10 (Minor)

### Scope

- `crates/fdemon-daemon/src/native_logs/mod.rs`: Add shared `EVENT_CHANNEL_CAPACITY` constant; add `#[derive(Clone)]` to both config structs
- `crates/fdemon-daemon/src/native_logs/android.rs`: Use shared constant; replace manual clone with `.clone()`
- `crates/fdemon-daemon/src/native_logs/macos.rs`: Use shared constant; replace manual clone with `.clone()`

### Details

#### Fix 1: Share `EVENT_CHANNEL_CAPACITY` (Issue #9)

`android.rs:26` defines:
```rust
const EVENT_CHANNEL_CAPACITY: usize = 256;
```

`macos.rs:254` hardcodes the same value:
```rust
let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(256);
```

**Fix:** Move the constant to `mod.rs` and reference it from both files.

In `native_logs/mod.rs`:
```rust
/// Capacity of the mpsc channel used to forward native log events from the
/// capture task to the TEA message loop. 256 provides headroom for bursty
/// log output without blocking the capture loop.
pub(crate) const EVENT_CHANNEL_CAPACITY: usize = 256;
```

In `android.rs`:
```rust
// Remove local `const EVENT_CHANNEL_CAPACITY`
let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(super::EVENT_CHANNEL_CAPACITY);
```

In `macos.rs`:
```rust
let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(super::EVENT_CHANNEL_CAPACITY);
```

#### Fix 2: Derive `Clone` on config structs (Issue #10)

Both structs are defined in `mod.rs` without `Clone`:

```rust
// mod.rs:68-81
pub struct AndroidLogConfig {
    pub device_serial: String,
    pub pid: Option<u32>,
    pub exclude_tags: Vec<String>,
    pub include_tags: Vec<String>,
    pub min_level: String,
}

// mod.rs:84-94
#[cfg(target_os = "macos")]
pub struct MacOsLogConfig {
    pub process_name: String,
    pub exclude_tags: Vec<String>,
    pub include_tags: Vec<String>,
    pub min_level: String,
}
```

Both are cloned field-by-field in their respective `spawn()` methods:

```rust
// android.rs:243-249
let config = AndroidLogConfig {
    device_serial: self.config.device_serial.clone(),
    pid: self.config.pid,
    exclude_tags: self.config.exclude_tags.clone(),
    include_tags: self.config.include_tags.clone(),
    min_level: self.config.min_level.clone(),
};

// macos.rs:248-253
let config = MacOsLogConfig {
    process_name: self.config.process_name.clone(),
    exclude_tags: self.config.exclude_tags.clone(),
    include_tags: self.config.include_tags.clone(),
    min_level: self.config.min_level.clone(),
};
```

All field types (`String`, `Vec<String>`, `Option<u32>`) implement `Clone`.

**Fix:** Add `#[derive(Clone)]` to both structs in `mod.rs`, then replace the manual construction blocks with `self.config.clone()`:

In `mod.rs`:
```rust
#[derive(Clone)]
pub struct AndroidLogConfig { /* ... */ }

#[cfg(target_os = "macos")]
#[derive(Clone)]
pub struct MacOsLogConfig { /* ... */ }
```

In `android.rs`:
```rust
let config = self.config.clone();
```

In `macos.rs`:
```rust
let config = self.config.clone();
```

### Acceptance Criteria

1. `EVENT_CHANNEL_CAPACITY` is defined once in `native_logs/mod.rs`
2. Both `android.rs` and `macos.rs` reference `super::EVENT_CHANNEL_CAPACITY`
3. No magic `256` literal remains in the channel construction
4. `AndroidLogConfig` and `MacOsLogConfig` both derive `Clone`
5. Manual field-by-field clone blocks are replaced with `.clone()`
6. `cargo check -p fdemon-daemon` passes
7. `cargo test -p fdemon-daemon --lib` passes
8. `cargo clippy -p fdemon-daemon -- -D warnings` passes

### Testing

No new tests needed — these are refactoring changes with identical runtime behavior. Existing tests cover the spawn logic and config usage.

### Notes

- Both changes are purely mechanical and safe. The `Clone` derive is idiomatic Rust and eliminates a maintenance hazard (forgetting to clone a new field produces a compile error with derive, but the manual approach would silently miss it too — both are safe, but derive is shorter and more conventional).
- The `EVENT_CHANNEL_CAPACITY` value of 256 is suitable for both platforms. If platform-specific tuning is needed later, the constant can be split back.
