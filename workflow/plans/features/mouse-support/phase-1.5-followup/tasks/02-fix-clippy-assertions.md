## Task: Fix `assertions_on_constants` clippy errors in `input_mouse.rs`

**Objective**: Resolve three `clippy::assertions_on_constants` errors that block `cargo clippy --workspace --all-targets -- -D warnings` and therefore Phase 1's stated success criteria.

**Depends on**: Task 01 (rename-click-to-press) — runs after Task 01 to avoid worktree-merge contention on `input_mouse.rs`.

**Estimated Time**: 0.25h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/input_mouse.rs`: Update `test_keymodset_none_is_empty` (lines 180–185) to bind `KeyModSet::NONE` to a local `let` before asserting.

**Files Read (Dependencies):**
- None.

### Details

The test currently reads:

```rust
#[test]
fn test_keymodset_none_is_empty() {
    assert!(!KeyModSet::NONE.shift);
    assert!(!KeyModSet::NONE.ctrl);
    assert!(!KeyModSet::NONE.alt);
}
```

Because `KeyModSet::NONE` is `pub const`, each assertion reduces to a constant boolean expression at compile time. `clippy::assertions_on_constants` rejects this under `-D warnings`.

Replace with a local `let` binding so the assertion target is no longer a constant expression from clippy's perspective:

```rust
#[test]
fn test_keymodset_none_is_empty() {
    let none = KeyModSet::NONE;
    assert!(!none.shift);
    assert!(!none.ctrl);
    assert!(!none.alt);
}
```

Do **not** use `#[allow(clippy::assertions_on_constants)]` — the lint exists to catch a real class of bugs (asserting on values that are constant when they should be runtime-bound), and a blanket allow would mask future regressions.

### Acceptance Criteria

1. `cargo clippy -p fdemon-app --all-targets -- -D warnings` passes with zero warnings.
2. `cargo clippy --workspace --all-targets -- -D warnings` passes with zero warnings.
3. `test_keymodset_none_is_empty` still passes and still verifies the same property (no field of `KeyModSet::NONE` is `true`).
4. No `#[allow(...)]` attribute was added.

### Testing

```bash
cargo clippy -p fdemon-app --all-targets -- -D warnings
cargo test -p fdemon-app input_mouse::tests::test_keymodset_none_is_empty
```

### Notes

- This issue was documented but unresolved at the end of Phase 1 (TASKS.md note in `phase-1-foundation/TASKS.md`).
- The fix is mechanical and self-contained. No risk to other tests.
