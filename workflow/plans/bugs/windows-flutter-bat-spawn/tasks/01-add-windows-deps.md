## Task: Add `which` and `dunce` dependencies

**Objective**: Add the two dependencies that the rest of the fix relies on: `which` (PATHEXT-aware executable lookup on Windows) and `dunce` (UNC `\\?\` prefix stripping).

**Depends on**: None

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `Cargo.toml` (workspace root): Add `which = "8"` and `dunce = "1"` to `[workspace.dependencies]`.
- `crates/fdemon-daemon/Cargo.toml`: Add `which = { workspace = true }` and `dunce = { workspace = true }` to its `[dependencies]`.

**Files Read (Dependencies):**
- None.

### Details

The workspace already uses `[workspace.dependencies]` for shared crate version pinning — follow that pattern.

```toml
# Cargo.toml (workspace root) — under [workspace.dependencies]
which = "8"
dunce = "1"
```

```toml
# crates/fdemon-daemon/Cargo.toml — under [dependencies]
which = { workspace = true }
dunce = { workspace = true }
```

### Acceptance Criteria

1. `cargo check --workspace` succeeds after the change (no other code uses these deps yet, so it's a no-op build-wise).
2. `cargo tree -p fdemon-daemon` shows `which v8.x` and `dunce v1.x`.
3. No version conflicts with existing dependencies.

### Testing

```bash
cargo check --workspace
cargo tree -p fdemon-daemon | grep -E "^├── (which|dunce) "
```

### Notes

- `which` MSRV is 1.70 (matches our minimum from `docs/DEVELOPMENT.md`).
- `dunce` is a tiny single-purpose crate (no transitive deps). It's the de-facto standard fix for Windows UNC-prefix issues.
- Pinning major-only (`"8"`, `"1"`) is consistent with how other deps are pinned in the workspace — confirm by reading `Cargo.toml` first.
- Do NOT add these deps to `fdemon-core`, `fdemon-app`, `fdemon-tui`, or the binary crate. Only `fdemon-daemon` owns process spawning.
