## Task: Add PTY Testing Dependencies

**Objective**: Add `expectrl` for PTY interaction testing and `insta` for snapshot testing to the project's dev-dependencies.

**Depends on**: None

### Scope

- `Cargo.toml`: Add new dev-dependencies

### Details

Add the following dependencies to the `[dev-dependencies]` section:

```toml
[dev-dependencies]
# Existing
tokio-test = "0.4"
tempfile = "3"
mockall = "0.13"

# NEW: PTY-based TUI testing
expectrl = "0.7"           # PTY spawn and interaction

# NEW: Snapshot testing
insta = { version = "1.41", features = ["filters"] }
```

**Dependency Rationale:**

| Crate | Purpose | Why this version |
|-------|---------|------------------|
| `expectrl` | PTY spawning and expect-style interaction | Stable, cross-platform PTY support |
| `insta` | Snapshot testing with inline reviews | Filters feature allows sanitizing timestamps/paths |

**Note on expectrl:**
- Supports spawning processes in a pseudo-terminal
- Provides `expect()` style pattern matching
- Works on Linux and macOS (Windows has limited support)
- Alternatives considered: `portable-pty` (lower-level), `pty-process` (less maintained)

### Acceptance Criteria

1. `cargo build` succeeds with new dependencies
2. `cargo test` runs without dependency conflicts
3. Both `expectrl` and `insta` are available for import in test code

### Testing

```bash
# Verify dependencies compile
cargo build

# Verify test dependencies are available
cargo test --test e2e -- --list

# Quick smoke test that imports work
echo '
#[test]
fn test_imports() {
    use expectrl::Session;
    use insta::assert_snapshot;
}
' | cargo test --test e2e
```

### Notes

- `insta` with `filters` feature allows redacting dynamic content (timestamps, UUIDs) from snapshots
- Consider adding `insta` CLI tool globally: `cargo install cargo-insta`
- The `cargo insta` command provides interactive snapshot review

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Implementation Details:**

(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo clippy` - Pending
- `cargo build` - Pending
- `cargo test` - Pending

**Notable Decisions:**
- (none yet)

**Risks/Limitations:**
- (none yet)
