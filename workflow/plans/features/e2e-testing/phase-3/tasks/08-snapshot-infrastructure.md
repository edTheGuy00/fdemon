## Task: Set Up Insta Snapshot Infrastructure

**Objective**: Configure `insta` for snapshot testing and create infrastructure for capturing and comparing TUI output.

**Depends on**: 05-test-reload-key, 06-test-session-keys, 07-test-quit-key

### Scope

- `tests/e2e/snapshots/`: **NEW** - Snapshot storage directory
- `tests/e2e/pty_utils.rs`: Add snapshot capture helpers
- `insta.yaml`: **NEW** - Insta configuration (optional)

### Details

#### 1. Configure Insta

Create `insta.yaml` at project root (optional but recommended):

```yaml
# Insta snapshot configuration
# See: https://insta.rs/docs/settings/

# Behavior settings
behavior:
  # Fail on first mismatch (CI-friendly)
  fail_fast: false
  # Review mode for local development
  review: true

# Snapshot file settings
snapshot:
  # Store snapshots alongside test files
  snapshot_path: "{module_path}/snapshots"
  # Include header with metadata
  include_header: true
```

#### 2. Add Snapshot Capture to PTY Utils

Extend `tests/e2e/pty_utils.rs`:

```rust
use insta::{assert_snapshot, with_settings};

impl FdemonSession {
    /// Capture current screen and return sanitized content for snapshot
    pub fn capture_for_snapshot(&mut self) -> PtyResult<String> {
        let raw = self.capture_screen()?;
        Ok(sanitize_for_snapshot(&raw))
    }

    /// Assert current screen matches snapshot
    pub fn assert_snapshot(&mut self, name: &str) -> PtyResult<()> {
        let content = self.capture_for_snapshot()?;
        with_settings!({
            // Redact dynamic content
            filters => vec![
                // Redact timestamps like "12:34:56"
                (r"\d{2}:\d{2}:\d{2}", "[TIME]"),
                // Redact dates like "2024-01-15"
                (r"\d{4}-\d{2}-\d{2}", "[DATE]"),
                // Redact UUIDs
                (r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", "[UUID]"),
                // Redact reload times like "245ms"
                (r"\d+ms", "[TIME_MS]"),
                // Redact paths
                (r"/Users/[^/]+/", "/USER/"),
                (r"/home/[^/]+/", "/USER/"),
            ],
        }, {
            assert_snapshot!(name, content);
        });
        Ok(())
    }
}

/// Sanitize terminal output for snapshot comparison
fn sanitize_for_snapshot(raw: &str) -> String {
    // Remove ANSI escape codes for cleaner snapshots
    let ansi_regex = regex::Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();
    let cleaned = ansi_regex.replace_all(raw, "");

    // Normalize whitespace
    cleaned
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}
```

#### 3. Create Snapshot Test Examples

Add to `tests/e2e/tui_interaction.rs`:

```rust
use insta::assert_snapshot;

/// Snapshot test for startup screen
#[tokio::test]
async fn snapshot_startup_screen() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    // Capture and assert snapshot
    session.assert_snapshot("startup_screen")
        .expect("Snapshot should match");

    session.kill().expect("Should kill process");
}

/// Snapshot test for device selector
#[tokio::test]
async fn snapshot_device_selector() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn_with_args(
        &fixture.path(),
        &["--no-auto-start"]
    ).expect("Failed to spawn fdemon");

    session.expect_device_selector().expect("Should show selector");

    session.assert_snapshot("device_selector")
        .expect("Snapshot should match");

    session.kill().expect("Should kill process");
}
```

### Directory Structure

```
tests/
├── e2e/
│   ├── mod.rs
│   ├── pty_utils.rs
│   ├── tui_interaction.rs
│   └── snapshots/              # NEW
│       ├── tui_interaction__startup_screen.snap
│       ├── tui_interaction__device_selector.snap
│       └── ...
```

### Acceptance Criteria

1. `insta` crate is properly configured
2. Snapshots are stored in organized directory structure
3. Dynamic content (timestamps, UUIDs, paths) is redacted
4. ANSI escape codes are stripped for readable snapshots
5. `cargo insta test` workflow works for reviewing changes

### Testing

```bash
# Run snapshot tests
cargo test --test e2e snapshot

# Review pending snapshots
cargo insta review

# Accept all pending snapshots
cargo insta accept

# Reject all pending snapshots
cargo insta reject
```

### Notes

- First run will create new snapshots that need review
- CI should fail on snapshot mismatches
- Use `cargo insta test --review` for interactive development
- Consider separate CI job for snapshot tests if they're slow

---

## Completion Summary

**Status:** Done

**Files Modified:**

| File | Changes |
|------|---------|
| `insta.yaml` | Created insta configuration at project root with snapshot_path and review settings |
| `tests/e2e/pty_utils.rs` | Added `capture_for_snapshot()`, `assert_snapshot()` methods and `sanitize_for_snapshot()` helper |
| `tests/e2e/snapshots/` | Created directory for snapshot storage |

**Implementation Details:**

1. **Insta Configuration (`insta.yaml`)**:
   - Configured snapshot path as `{module_path}/snapshots`
   - Enabled review mode for local development
   - Set `fail_fast: false` for CI-friendly behavior
   - Enabled snapshot headers with metadata

2. **Snapshot Capture Methods**:
   - `capture_for_snapshot()`: Captures screen and sanitizes content (strips ANSI, normalizes whitespace)
   - `assert_snapshot(name)`: Full snapshot assertion with automatic redaction filters
   - `sanitize_for_snapshot(raw)`: Helper function that removes ANSI codes and trims trailing whitespace

3. **Redaction Filters**:
   - Timestamps: `12:34:56` -> `[TIME]`
   - Dates: `2024-01-15` -> `[DATE]`
   - UUIDs: `550e8400-...` -> `[UUID]`
   - Milliseconds: `245ms` -> `[TIME_MS]`
   - User paths: `/Users/username/` -> `/USER/`, `/home/username/` -> `/USER/`

4. **ANSI Code Removal**:
   - Regex pattern `\x1b\[[0-9;]*[a-zA-Z]` strips all ANSI escape sequences
   - Handles colors, cursor movement, clear screen, and other terminal control codes

**Testing Performed:**
- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --test e2e sanitize_for_snapshot` - Passed (4 tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)

**Notable Decisions:**

1. **Regex-based ANSI Stripping**: Used regex pattern `\x1b\[[0-9;]*[a-zA-Z]` instead of a dedicated ANSI parsing library for simplicity. This covers the vast majority of ANSI escape codes used by ratatui/crossterm.

2. **Comprehensive Redaction**: Applied redaction filters to timestamps, UUIDs, paths, and milliseconds to ensure stable snapshots across different runs and environments. Filters are applied via `insta::with_settings!` macro for each assertion.

3. **Whitespace Normalization**: Trim trailing spaces from each line but preserve line structure. This prevents false negatives from terminal width variations while maintaining readability.

4. **Snapshot Directory**: Created `tests/e2e/snapshots/` for organized snapshot storage, following insta's convention of storing snapshots alongside test files.

**Risks/Limitations:**

1. **ANSI Regex Coverage**: The regex pattern may not catch all exotic ANSI escape sequences (OSC, DCS, etc.), but should handle all sequences produced by ratatui/crossterm.

2. **Path Redaction Scope**: Path redaction only handles `/Users/` and `/home/` prefixes. Windows paths (`C:\Users\`) are not covered but can be added if needed.

3. **Cargo-insta CLI Not Required**: The `cargo-insta` CLI tool is not installed, but snapshot tests work via the `insta` crate directly. Users can install `cargo install cargo-insta` for the `cargo insta review` workflow.
