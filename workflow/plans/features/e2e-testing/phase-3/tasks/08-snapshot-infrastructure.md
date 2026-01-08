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

**Status:** Not Started

**Files Modified:**
- (none yet)

**Implementation Details:**

(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo clippy` - Pending
- `cargo test` - Pending
- `cargo insta test` - Pending

**Notable Decisions:**
- (none yet)

**Risks/Limitations:**
- (none yet)
