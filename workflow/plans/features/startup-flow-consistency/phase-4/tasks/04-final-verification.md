## Task: Final Verification

**Objective**: Perform final verification that the complete startup flow consistency feature works correctly and all quality gates pass.

**Depends on**: 02-update-snapshot-tests, 03-update-documentation

**Estimated Time**: 0.5 hours

### Scope

- Full test suite verification
- Final manual testing
- Feature completion sign-off

### Quality Gates

Run the full verification suite:

```bash
# Format check
cargo fmt --check

# Compilation check
cargo check

# All tests
cargo test

# Lints with warnings as errors
cargo clippy -- -D warnings
```

**All must pass with zero errors or warnings.**

### Manual Verification Checklist

#### Scenario 1: Fresh Start with Auto-Start

- [ ] Delete `.fdemon/settings.local.toml` if it exists
- [ ] Ensure `auto_start = true` in config
- [ ] Run app with device connected
- [ ] Verify: Normal UI shown immediately with loading overlay on top
- [ ] Verify: Loading messages cycle (every ~1.5s)
- [ ] Verify: Session starts, overlay disappears, no device selector shown

#### Scenario 2: Fresh Start without Auto-Start

- [ ] Ensure `auto_start = false` or no config
- [ ] Run app
- [ ] Verify: Normal mode ("Not Connected")
- [ ] Press '+' → StartupDialog appears
- [ ] Select device → Session starts

#### Scenario 3: Remembered Selection

- [ ] Ensure `auto_start = true`
- [ ] Run app, let session start
- [ ] Quit app
- [ ] Run app again
- [ ] Verify: Same device/config used automatically

#### Scenario 4: Error Handling

- [ ] Disconnect all devices
- [ ] Ensure `auto_start = true`
- [ ] Run app
- [ ] Verify: Loading → StartupDialog with error

### Code Quality Checklist

- [ ] No `#[allow(dead_code)]` attributes remaining
- [ ] No TODO comments for this feature
- [ ] No println! debugging left in code
- [ ] All new public functions have doc comments
- [ ] Code follows project style (check CODE_STANDARDS.md)

### Documentation Checklist

- [ ] ARCHITECTURE.md updated
- [ ] CLAUDE.md reviewed (updated if needed)
- [ ] Plan document reflects actual implementation
- [ ] Task completion summaries filled in

### Feature Sign-Off

When all checks pass:

1. Update PLAN.md with completion status
2. Move plan to completed (if project uses that convention)
3. Consider creating a PR summary

### Acceptance Criteria

1. `cargo fmt --check` passes
2. `cargo check` passes
3. `cargo test` passes (all tests)
4. `cargo clippy -- -D warnings` passes
5. All manual scenarios verified
6. Documentation is accurate
7. No outstanding TODOs for this feature

---

## Completion Summary

**Status:** Not Started

**Quality Gates:**
- [ ] cargo fmt --check
- [ ] cargo check
- [ ] cargo test
- [ ] cargo clippy -- -D warnings

**Manual Scenarios:**
- [ ] Fresh start with auto-start
- [ ] Fresh start without auto-start
- [ ] Remembered selection
- [ ] Error handling

**Sign-Off:**

**Date:** (pending)

**Notes:**

(pending)
