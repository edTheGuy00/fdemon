## Task: Guard Message Clone Behind Plugins Check

**Objective**: Avoid unconditionally cloning the `Message` on every `process_message()` call when no plugins are registered. Most users will have zero plugins.

**Depends on**: None

**Severity**: MAJOR (unnecessary clone on hot path)

**Source**: Code Quality Inspector, Logic & Reasoning Checker (ACTION_ITEMS.md Major #2)

### Scope

- `crates/fdemon-app/src/engine.rs:236`: Guard `msg.clone()` with `self.plugins.is_empty()` check

### Details

**Current code (engine.rs:231-255):**
```rust
pub fn process_message(&mut self, msg: Message) {
    let pre = StateSnapshot::capture(&self.state);

    let msg_for_plugins = msg.clone();  // <-- always clones, even with 0 plugins

    crate::process::process_message(/* ... msg moved here ... */);

    let post = StateSnapshot::capture(&self.state);
    self.emit_events(&pre, &post);
    self.notify_plugins_message(&msg_for_plugins);
}
```

The `Message` enum can contain `Vec<LogEntry>`, device lists, and other data that is expensive to clone. This clone happens on **every single message** processed, including high-frequency log entries. When `self.plugins` is empty (the default), the clone is wasted -- `notify_plugins_message` loops over an empty vec.

**Fix:**
```rust
let msg_for_plugins = if self.plugins.is_empty() {
    None
} else {
    Some(msg.clone())
};

// ... process_message(msg) ...

if let Some(ref m) = msg_for_plugins {
    self.notify_plugins_message(m);
}
```

### Acceptance Criteria

1. `msg.clone()` only happens when `self.plugins` is non-empty
2. `notify_plugins_message()` only called when there are plugins
3. Behavior is identical when plugins ARE registered
4. `cargo check -p fdemon-app` passes
5. `cargo test -p fdemon-app --lib` passes

### Testing

```bash
cargo check -p fdemon-app
cargo test -p fdemon-app --lib
```

### Notes

- This is a hot-path optimization. In typical usage with no plugins, this avoids cloning every `Message::Daemon(DaemonEvent::Stdout(...))` which can be frequent during Flutter log output.
- The `notify_plugins_message` method (line ~511) already handles the empty case gracefully, so the guard is purely about avoiding the clone cost.

---

## Completion Summary

**Status:** Not started
