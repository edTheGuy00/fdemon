# Task 04: PTY test gating consistency

**Severity:** LATENT — currently does not break CI; included for symmetry to prevent future surprises.

**Estimated Time:** 0.25 hours

## Objective

In commit `ba8aa79` we added per-test `#[cfg_attr(target_os = "windows", ignore)]` to 11 PTY tests in `tests/e2e/tui_interaction.rs` and `tests/e2e/tui_workflows.rs` because Windows ConPTY's behaviour breaks `expectrl`'s pattern matching. Two sibling test files — `tests/e2e/settings_page.rs` and `tests/e2e/debug_settings.rs` — also use `FdemonSession::spawn` (which wraps `expectrl::Session::spawn`) but have NOT been Windows-gated. They are protected today only because every test in those files carries an unrelated `#[ignore]` attribute, so they don't run on any platform.

If anyone removes one of those `#[ignore]`s in the future to enable a previously-skipped test, it will fail on Windows immediately. Add the Windows gating now while we're auditing the surface, so the symmetry holds.

**Depends on:** None

## Scope

**Files Modified (Write):**
- `tests/e2e/settings_page.rs` — add `#[cfg_attr(target_os = "windows", ignore = "...")]` to every `#[tokio::test]` that calls `FdemonSession::spawn`
- `tests/e2e/debug_settings.rs` — same, for the single test in that file

**Files Read (Dependencies):**
- `tests/e2e/tui_interaction.rs` — for the existing pattern's exact wording (the `cfg_attr` ignore reason should match what's already there)

## Details

The pattern to apply (already in `tui_interaction.rs` and `tui_workflows.rs`):

```rust
#[tokio::test]
#[serial]
#[cfg_attr(
    target_os = "windows",
    ignore = "PTY regex matching on Windows ConPTY is unreliable; TUI rendering verified by widget unit tests"
)]
async fn test_name() { ... }
```

The `#[cfg_attr]` should be placed AFTER any existing `#[serial]` and the existing per-test `#[ignore = "..."]`. Because both Windows-gating and the existing test-skip reason share the `ignore` attribute, two `#[ignore]`-style attributes coexist cleanly: the test stays ignored on every platform, but the Windows runner specifically reports the ConPTY reason rather than the unrelated one.

**Implementor steps:**

1. Open `tests/e2e/settings_page.rs`. For every function annotated `#[tokio::test]` that calls `FdemonSession::spawn(...)` directly or via a helper, add the `#[cfg_attr(target_os = "windows", ignore = ...)]` line. Use grep `grep -n "FdemonSession::spawn\|spawn_headless" tests/e2e/settings_page.rs` to enumerate.
2. Open `tests/e2e/debug_settings.rs` (a single `#[tokio::test]` exists at line 11). Add the same attribute.
3. Verify on macOS that nothing changes — `cargo test --test e2e settings_page` and `cargo test --test e2e debug_settings` should report the same ignored counts before and after.

**Do not change** the per-test `#[ignore = "..."]` attributes already present — those exist for unrelated reasons (real-Flutter dependency, headless mode unsuitability, etc.) and are independent of the Windows ConPTY issue.

## Acceptance Criteria

- [ ] Every `#[tokio::test]` in `tests/e2e/settings_page.rs` that calls `FdemonSession::spawn` (or a helper that does) carries `#[cfg_attr(target_os = "windows", ignore = "PTY regex matching on Windows ConPTY is unreliable; TUI rendering verified by widget unit tests")]`.
- [ ] The same attribute is on the test in `tests/e2e/debug_settings.rs`.
- [ ] No existing `#[ignore = "..."]` attributes are removed.
- [ ] `cargo test --test e2e` reports the same ignored test count on macOS as it did before this task (the new attribute is dead code on non-Windows, so behaviour is unchanged on macOS/Linux).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] `cargo fmt --all -- --check` clean.

## Out of Scope

- Removing or restructuring the existing `#[ignore]` attributes.
- Adding new Windows-compatible PTY tests.
- Refactoring `pty_utils.rs` to be Windows-compilable. (It already is — `expectrl` has a ConPTY backend; the runtime regex-matching is what's unreliable, and that's what the per-test ignore handles.)

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/settings_page.rs` | Added `#[cfg_attr(target_os = "windows", ignore = "PTY regex matching on Windows ConPTY is unreliable; TUI rendering verified by widget unit tests")]` after the existing `#[ignore]` on all 28 `#[tokio::test]` functions |
| `tests/e2e/debug_settings.rs` | Added the same `#[cfg_attr]` to the single `#[tokio::test]` (`debug_settings_rendering`) |

### Notable Decisions/Tradeoffs

1. **Placement after existing `#[ignore]`**: The `#[cfg_attr]` is placed after the existing per-test `#[ignore]` attribute as specified in the task. On non-Windows platforms the first `#[ignore]` takes effect and the Windows reason is inert dead code. On Windows the ConPTY reason surfaces instead.
2. **`debug_settings.rs` not in module tree**: The file is not currently wired into `tests/e2e.rs` (no `mod debug_settings;` declaration), so the test doesn't currently run anywhere. The annotation was added proactively as specified so it's correct if/when it gets included.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)
- `cargo test --test e2e settings_page` - 28 ignored, 0 passed, 0 failed (same count as before)

### Risks/Limitations

1. **Dead annotation in debug_settings.rs**: Since `debug_settings.rs` is not included in the test module, the added `#[cfg_attr]` has no runtime effect yet. This is intentional — it's proactive gating as described in the task.
