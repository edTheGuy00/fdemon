## Task: Add doc comments to public functions in `event.rs`

**Objective**: Bring `crates/fdemon-tui/src/event.rs` into compliance with `docs/CODE_STANDARDS.md` ("All `pub` functions and types must have `///` doc comments") by documenting the two public functions and expanding the sparse module-level header.

**Depends on**: Task 01 (rename-click-to-press) — Task 01 also writes `event.rs`; running after avoids worktree-merge contention.

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/event.rs`:
  - Replace the single-line `//! Terminal event polling` header with a multi-line `//!` block describing all three responsibilities (key conversion, mouse conversion, polling).
  - Add `///` doc comments to `pub fn key_event_to_input` (around line 89).
  - Add `///` doc comments to `pub fn poll` (around line 116).

**Files Read (Dependencies):**
- None.

### Details

`docs/CODE_STANDARDS.md` requires `///` headers on all `pub` items including purpose, return semantics, and edge cases.

**Module header** — replace:

```rust
//! Terminal event polling
```

with something like:

```rust
//! Terminal event handling for the TUI.
//!
//! This module polls crossterm for terminal events, converts keyboard and
//! mouse events into the abstract [`InputKey`] / [`MouseInput`] types defined
//! in `fdemon-app`, and emits the corresponding [`Message`] variants onto the
//! TEA bus. `Moved` mouse events are dropped at this boundary (high volume,
//! no consumer); all other event kinds are exhaustively mapped.
```

**`pub fn key_event_to_input`** — example header:

```rust
/// Convert a crossterm [`KeyEvent`] into the abstract [`InputKey`] used by
/// the TEA handler layer.
///
/// Returns `None` for key codes not represented in [`InputKey`] (e.g. Insert,
/// F13+). Callers should pass only `KeyEventKind::Press` events; key repeats
/// and releases are filtered earlier in [`poll`].
pub fn key_event_to_input(key: KeyEvent) -> Option<InputKey> { ... }
```

**`pub fn poll`** — example header:

```rust
/// Poll the terminal for the next available event with a short timeout.
///
/// Returns:
/// * `Ok(Some(Message))` — a translated key, mouse, or resize event
/// * `Ok(None)` — the timeout elapsed with no event, or the event was filtered
///   (e.g. `KeyEventKind::Repeat`, `MouseEventKind::Moved`)
/// * `Err(_)` — an I/O error from crossterm; callers should treat this as fatal
///
/// This is the single integration point between crossterm and the TEA loop.
/// All event filtering happens here so the engine never sees raw terminal
/// events.
pub fn poll() -> Result<Option<Message>> { ... }
```

Adjust wording to match the actual function signatures and the project's documentation style (skim a few other `pub fn` doc comments in `fdemon-tui` for the prevailing voice).

### Acceptance Criteria

1. `crates/fdemon-tui/src/event.rs` has a multi-line `//!` module header that mentions key conversion, mouse conversion, and polling responsibilities.
2. `pub fn key_event_to_input` has a `///` doc comment that includes purpose, return semantics, and at least one edge case (e.g. unmapped key codes).
3. `pub fn poll` has a `///` doc comment that includes purpose, the meaning of each return arm (`Ok(Some)` / `Ok(None)` / `Err`), and the filtering responsibility.
4. `cargo doc -p fdemon-tui --no-deps` builds without doc-link errors (you can run this locally; not required by the standard CI gate).
5. `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

No new tests required. Verify with:

```bash
cargo doc -p fdemon-tui --no-deps
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

### Notes

- The two `pub(crate)` helpers (`key_modifiers_to_set`, `mouse_event_to_input`) are intentionally not in scope — the standard applies to `pub` items, and these are crate-internal. You may add brief `///` comments to them if you wish, but it is not required.
