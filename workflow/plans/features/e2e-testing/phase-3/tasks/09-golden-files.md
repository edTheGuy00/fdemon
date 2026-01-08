## Task: Create Golden Files for Key UI States

**Objective**: Create and commit snapshot golden files for all critical UI states to enable visual regression detection.

**Depends on**: 08-snapshot-infrastructure

### Scope

- `tests/e2e/snapshots/`: Create golden files for key states
- `tests/e2e/tui_interaction.rs`: Add comprehensive snapshot tests

### Details

#### Golden Files to Create

| Snapshot Name | UI State | Description |
|---------------|----------|-------------|
| `startup_screen` | Initial | Header + loading indicator |
| `device_selector` | Modal | Device/emulator selection list |
| `device_selector_empty` | Modal | "No devices found" state |
| `running_state` | Normal | App running with status bar |
| `reloading_state` | Transient | Hot reload in progress |
| `error_state` | Error | Compilation error display |
| `quit_confirmation` | Dialog | Quit confirmation prompt |
| `multi_session_tabs` | Normal | Tab bar with multiple sessions |
| `log_view_scrolled` | Normal | Log view after scrolling |

#### Snapshot Tests

Add comprehensive snapshot tests to `tests/e2e/tui_interaction.rs`:

```rust
mod snapshot_tests {
    use super::*;

    /// Golden file: Initial startup screen
    #[tokio::test]
    async fn golden_startup_screen() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path())
            .expect("Failed to spawn fdemon");

        session.expect_header().expect("Should show header");
        session.assert_snapshot("startup_screen").unwrap();

        session.kill().unwrap();
    }

    /// Golden file: Device selector modal
    #[tokio::test]
    async fn golden_device_selector() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn_with_args(
            &fixture.path(),
            &["--no-auto-start"]
        ).expect("Failed to spawn fdemon");

        session.expect_device_selector().expect("Should show selector");
        session.assert_snapshot("device_selector").unwrap();

        session.kill().unwrap();
    }

    /// Golden file: Running state with status bar
    #[tokio::test]
    async fn golden_running_state() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path())
            .expect("Failed to spawn fdemon");

        session.expect_running().expect("Should be running");
        // Wait a moment for status to stabilize
        tokio::time::sleep(Duration::from_millis(500)).await;

        session.assert_snapshot("running_state").unwrap();

        session.kill().unwrap();
    }

    /// Golden file: Reloading state
    #[tokio::test]
    async fn golden_reloading_state() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path())
            .expect("Failed to spawn fdemon");

        session.expect_running().expect("Should be running");
        session.send_key('r').expect("Send reload");
        session.expect_reloading().expect("Should be reloading");

        session.assert_snapshot("reloading_state").unwrap();

        session.kill().unwrap();
    }

    /// Golden file: Error state (compilation error)
    #[tokio::test]
    async fn golden_error_state() {
        let fixture = TestFixture::error_app();
        let mut session = FdemonSession::spawn(&fixture.path())
            .expect("Failed to spawn fdemon");

        // Error app should show compilation error
        session.expect("error|Error|failed|Failed")
            .expect("Should show error");

        session.assert_snapshot("error_state").unwrap();

        session.kill().unwrap();
    }

    /// Golden file: Quit confirmation dialog
    #[tokio::test]
    async fn golden_quit_confirmation() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path())
            .expect("Failed to spawn fdemon");

        session.expect_header().expect("Should show header");
        session.send_key('q').expect("Send quit");
        session.expect("quit|Quit").expect("Should show confirmation");

        session.assert_snapshot("quit_confirmation").unwrap();

        session.send_key('n').expect("Cancel quit");
        session.kill().unwrap();
    }
}
```

### Workflow for Creating Golden Files

1. **Run tests to generate snapshots:**
   ```bash
   cargo test --test e2e golden_ -- --nocapture
   ```

2. **Review generated snapshots:**
   ```bash
   cargo insta review
   ```

3. **Accept correct snapshots:**
   ```bash
   cargo insta accept
   ```

4. **Commit golden files:**
   ```bash
   git add tests/e2e/snapshots/
   git commit -m "chore(e2e): add golden files for TUI snapshots"
   ```

### CI Integration

Add to `.github/workflows/e2e.yml`:

```yaml
- name: Run snapshot tests
  run: |
    cargo test --test e2e snapshot -- --nocapture
    # Fail if any snapshots are pending review
    cargo insta test --check
```

### Acceptance Criteria

1. Golden files exist for all 9 key UI states
2. Snapshots are committed to version control
3. CI fails when snapshots don't match
4. Dynamic content is properly redacted
5. Snapshots are human-readable (no ANSI codes)

### Testing

```bash
# Generate all golden files
cargo test --test e2e golden_

# Verify no pending changes
cargo insta test --check

# Interactive review
cargo insta review
```

### Notes

- Golden files should be reviewed by humans before committing
- Large UI changes will require snapshot updates
- Consider separate PR for snapshot-only changes
- Document process for updating golden files in CONTRIBUTING.md

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
- `cargo insta test --check` - Pending

**Notable Decisions:**
- (none yet)

**Risks/Limitations:**
- (none yet)
