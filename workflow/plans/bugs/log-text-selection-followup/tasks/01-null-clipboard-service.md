## Task: NullClipboard service + cfg-gate MemoryClipboard

**Objective:** Add a `NullClipboard` whose `write_text` returns `Err(Error::terminal("system clipboard unavailable"))`, and gate `MemoryClipboard` behind `#[cfg(test)]` so it cannot be used in production paths. Re-export `NullClipboard` from the `services` module.

**Depends on:** None

**Agent:** implementor

**Estimated time:** 1-1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/services/clipboard.rs`: Add `NullClipboard` struct + `Clipboard` impl. Gate `MemoryClipboard` and its `Default` impl behind `#[cfg(test)]`.
- `crates/fdemon-app/src/services/mod.rs`: Re-export `NullClipboard` from the public API; keep `MemoryClipboard` re-export `#[cfg(test)]` only.

**Files Read (Dependencies):**
- `crates/fdemon-core/src/error.rs`: confirm `Error::terminal(impl Into<String>) -> Error` constructor (already used by tests via `FailingClipboard`).

### Details

The new struct is intentionally trivial:

```rust
/// Clipboard impl used when the OS clipboard is unavailable at runtime
/// (e.g. headless Linux without X/Wayland, ssh without forwarding,
/// sandboxed environment). Every write returns an error so the runner's
/// failure-toast path fires and the user sees that copy is non-functional.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullClipboard;

impl Clipboard for NullClipboard {
    fn write_text(&mut self, _text: &str) -> Result<()> {
        Err(Error::terminal("system clipboard unavailable"))
    }
}
```

`MemoryClipboard` move:

- Wrap the struct definition AND its `Clipboard` impl AND its `Default` impl in `#[cfg(test)]`.
- The two existing unit tests (`test_memory_clipboard_records_writes`, `test_memory_clipboard_returns_ok`) stay where they are (they're already inside `#[cfg(test)] mod tests`).
- In `services/mod.rs`, change `pub use clipboard::{Clipboard, MemoryClipboard, SystemClipboard};` to:
  - `pub use clipboard::{Clipboard, NullClipboard, SystemClipboard};`
  - `#[cfg(test)] pub use clipboard::MemoryClipboard;`

### Acceptance Criteria

1. `NullClipboard::default().write_text("anything")` returns `Err(_)` whose message contains "system clipboard unavailable".
2. `MemoryClipboard` is not constructible from `cargo build --release` (i.e. with no test cfg). Verify by running `cargo check --workspace --all-targets --release`.
3. Existing `MemoryClipboard` tests still pass under `cargo test --workspace`.
4. New unit test `test_null_clipboard_returns_err` lives in `clipboard.rs` and asserts the error path.

### Testing

Add one new unit test in `crates/fdemon-app/src/services/clipboard.rs`:

```rust
#[cfg(test)]
mod tests {
    // ... existing tests ...

    #[test]
    fn test_null_clipboard_returns_err() {
        let mut c = NullClipboard;
        let result = c.write_text("hello");
        assert!(result.is_err(), "NullClipboard must return Err");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("system clipboard unavailable"),
            "error message must mention unavailability; got: {err_msg}"
        );
    }
}
```

### Notes

- Task 08 will adopt `NullClipboard` at the three runner fallback sites (`runner.rs:31-37, 142-148, 213`).
- Do NOT modify any code outside `services/`. The runner-side change belongs to Task 08.
- The `#[cfg(test)]` gate prevents downstream crates (and a future headless runner) from accidentally substituting `MemoryClipboard` in production.
