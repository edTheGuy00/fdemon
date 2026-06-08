## Task: Remove redundant `missing_binaries.clone()` (N1) — OPTIONAL

**Severity:** NITPICK (optional — does NOT fail the quality gate)

**Objective**: Replace an unnecessary `Vec<String>` clone with a move in
`check_linux_prerequisites`, per `docs/CODE_STANDARDS.md` "avoid unnecessary clones".

**Depends on**: None

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`

### Details

`prerequisites.rs:261`:

```rust
} else if !missing_binaries.is_empty() {
    let mut all_missing = missing_binaries.clone();
    if gtk_missing {
        all_missing.push(GTK_ITEM_LABEL.to_string());
    }
    ComponentCheck { /* … uses all_missing … */ }
}
```

`missing_binaries` is a locally-owned `Vec<String>` that is **not used again** after this
branch (each branch is terminal — it constructs and returns a `ComponentCheck`). The
`.clone()` allocates a full copy for no reason. Move it instead:

```rust
} else if !missing_binaries.is_empty() {
    let mut all_missing = missing_binaries;
    if gtk_missing {
        all_missing.push(GTK_ITEM_LABEL.to_string());
    }
    ComponentCheck { /* … */ }
}
```

**Important:** This is **not** a gate blocker. `clippy::redundant_clone` is a nursery
lint (allow-by-default); a forced fresh `cargo clippy -p fdemon-daemon --all-targets --
-D warnings` is already clean. This is a pure style/idiom cleanup. Verify the borrow
checker is satisfied after the move (no later use of `missing_binaries` in that branch —
there is none today).

### Acceptance Criteria

1. The `.clone()` at `prerequisites.rs:261` is replaced by a move (or otherwise avoided).
2. No other branch references `missing_binaries` after the move (compile-verified).
3. `cargo test -p fdemon-daemon` and the full workspace gate stay green; detection
   behavior is unchanged.

### Testing

Existing `check_linux_prerequisites` / `build_linux_check_from_missing` tests cover the
behavior; no new test needed. Run `cargo clippy --workspace --all-targets -- -D warnings`
to confirm no regression.

### Notes

- Optional NITPICK — may be deferred without blocking. Touches only `prerequisites.rs`;
  parallel-safe with tasks 01 and 05.
- Do **not** also "collapse" the `else { if *tool == "pkg-config" }` block at `:227-231`:
  the reviewer claim that it fails `collapsible_else_if` was verified FALSE (clippy-clean).

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` | Replaced `missing_binaries.clone()` with move at line 261 |

### Notable Decisions/Tradeoffs

1. **Move vs clone**: Confirmed `missing_binaries` is not referenced after the move point — all three `if/else if/else` branches are terminal (each constructs and returns a `ComponentCheck`). The borrow checker accepts the move without issue.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all 6,875+ tests pass)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. None — single-character idiom change with no behavior impact.
