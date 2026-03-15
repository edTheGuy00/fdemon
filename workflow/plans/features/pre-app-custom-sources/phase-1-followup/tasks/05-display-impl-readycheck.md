## Task: Move `describe_ready_check` to `Display` Impl on `ReadyCheck`

**Objective**: Replace the private free function `describe_ready_check` in `native_logs.rs` with a `Display` impl on `ReadyCheck` in `config/types.rs`.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/config/types.rs`: Add `impl Display for ReadyCheck`
- `crates/fdemon-app/src/actions/native_logs.rs`: Remove `describe_ready_check`, use `format!("{}", check)` or `check.to_string()`

### Details

#### Current Code (`native_logs.rs:703-712`)

```rust
fn describe_ready_check(check: &ReadyCheck) -> String {
    match check {
        ReadyCheck::Http { url, .. } => format!("http: {}", url),
        ReadyCheck::Tcp { host, port, .. } => format!("tcp: {}:{}", host, port),
        ReadyCheck::Command { command, .. } => format!("command: {}", command),
        ReadyCheck::Stdout { pattern, .. } => format!("stdout: /{}/", pattern),
        ReadyCheck::Delay { seconds } => format!("delay: {}s", seconds),
    }
}
```

#### Target: `impl Display` in `config/types.rs`

```rust
impl std::fmt::Display for ReadyCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadyCheck::Http { url, .. } => write!(f, "http: {}", url),
            ReadyCheck::Tcp { host, port, .. } => write!(f, "tcp: {}:{}", host, port),
            ReadyCheck::Command { command, .. } => write!(f, "command: {}", command),
            ReadyCheck::Stdout { pattern, .. } => write!(f, "stdout: /{}/", pattern),
            ReadyCheck::Delay { seconds } => write!(f, "delay: {}s", seconds),
        }
    }
}
```

Then in `native_logs.rs`, replace `describe_ready_check(&check)` with `check.to_string()`.

### Acceptance Criteria

1. `ReadyCheck` implements `Display` in `config/types.rs`
2. Free function `describe_ready_check` is removed from `native_logs.rs`
3. Call sites updated to use `.to_string()` or `format!("{}", check)`
4. All tests pass

### Notes

- `describe_ready_check` is called at one site: line 606 inside the `join_set.spawn` closure in `spawn_pre_app_sources`
- The `Display` impl lives alongside the existing `impl ReadyCheck` block that has `validate()`
