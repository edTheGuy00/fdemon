## Task: Rename `MouseInput::Click` → `MouseInput::Press`

**Objective**: Rename the `MouseInput::Click` variant to `MouseInput::Press` so the variant name matches its semantics (it is emitted on `MouseEventKind::Down`, which is button-press, not a debounced click). Run before any other Phase 1.5 task so all subsequent work uses the corrected name.

**Depends on**: None

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/input_mouse.rs`: Rename the `Click` variant in the `MouseInput` enum (around line 60–70). Update the `position()` and `modifiers()` methods if they pattern-match the variant name. Update all in-file unit tests that construct `MouseInput::Click { ... }` (tests around lines 124–217).
- `crates/fdemon-tui/src/event.rs`: Update the `mouse_event_to_input` match arm that maps `MouseEventKind::Down(...)` from `MouseInput::Click {...}` to `MouseInput::Press {...}`. Update any unit tests that assert on the variant name (tests around lines 269–423).
- `crates/fdemon-app/src/handler/mouse.rs`: Update the `assert_noop` test helper and any test that constructs `MouseInput::Click { ... }`.

**Files Read (Dependencies):**
- None. This is a pure rename.

### Details

The PR review (`workflow/reviews/features/mouse-support-phase-1-foundation/REVIEW.md` finding N3) flagged that `Click` is misleading because crossterm distinguishes `Down` (press) from a debounced `Click` (down+up sequence). The variant is emitted on `MouseEventKind::Down`, so `Press` is the accurate name. Phase 2+ will start consuming this variant for hit-testing; renaming now is cheap because there are zero consumers in production code today.

Mechanical change. Suggested approach:

1. In `crates/fdemon-app/src/input_mouse.rs`, rename:
   - The variant: `Click { x: u16, y: u16, button: MouseButton, modifiers: KeyModSet }` → `Press { ... }`
   - All match arms in `position()` and `modifiers()` if they spell out the variant name (or-patterns may already be exhaustive without naming `Click` explicitly — check).
   - All test constructors and assertions that mention `MouseInput::Click`.

2. In `crates/fdemon-tui/src/event.rs`:
   - The match arm in `mouse_event_to_input` that handles `MouseEventKind::Down(button)` constructs `MouseInput::Click { ... }` — change to `MouseInput::Press { ... }`.
   - Tests around lines 269–423 that assert on `MouseInput::Click` patterns must be updated.

3. In `crates/fdemon-app/src/handler/mouse.rs`:
   - The `assert_noop` test helper and any tests constructing `MouseInput::Click` must use `Press`.

Verify nothing was missed by grepping the workspace for `MouseInput::Click` after the rename:

```bash
grep -r "MouseInput::Click" crates/ tests/
# Should produce zero results.
```

### Acceptance Criteria

1. `MouseInput` enum exports a variant named `Press`, not `Click`. Field shape (`x`, `y`, `button`, `modifiers`) is unchanged.
2. `mouse_event_to_input` produces `MouseInput::Press {...}` from `MouseEventKind::Down(button)`.
3. All existing unit tests pass (no semantic change beyond the name).
4. `grep -r "MouseInput::Click" crates/ tests/` returns no matches.
5. `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` all pass. *(Note: clippy may still fail on the pre-existing `assertions_on_constants` issue in `input_mouse.rs:182-184`; that is fixed by Task 02 and not by this task.)*

### Testing

No new tests required. Run the existing suite:

```bash
cargo test -p fdemon-app input_mouse
cargo test -p fdemon-app handler::mouse
cargo test -p fdemon-tui event
```

All three should pass without modification beyond the rename.

### Notes

- This is purely additive forward-compatibility hygiene. No public API has consumers yet; renaming is risk-free.
- If any other crate (e.g., headless mode, MCP integration) ever references `MouseInput::Click`, this rename would break it — but as of `feat/mouse-support` HEAD, no such consumer exists.
