# Task 06: Build verification

**Files:** None (verification only)
**Depends on:** Tasks 01–05 merged
**Wave:** 2

## What to do

Run the full quality gate from `docs/DEVELOPMENT.md`:

```bash
cargo fmt --all -- --check && \
  cargo check --workspace --all-targets && \
  cargo test --workspace && \
  cargo clippy --workspace --all-targets -- -D warnings
```

All four steps must pass.

## Manual smoke tests

1. **Bug 1 — mode buttons clickable.**
   - Open the New Session dialog.
   - Click Debug button → selection moves to Debug.
   - Click Profile button → selection moves to Profile.
   - Click Release button → selection moves to Release.
   - Repeat for an FDemon config (mode editable) and a VSCode config (mode
     non-editable) — for the latter, clicks must be silently ignored.

2. **Bug 2 — pointer shape.** (Requires a terminal that supports OSC 22:
   kitty / Ghostty / Foot / Alacritty with `terminal.osc22 = true`.)
   - Launch fdemon. Hover over the TUI. Pointer should be an arrow, not
     I-beam.
   - Exit fdemon. Pointer should revert to terminal default.
   - On an unsupported terminal (iTerm2 / Terminal.app), confirm no
     visible garbage appears at launch or exit (sequence must be silently
     discarded).

3. **Bug 3 — no SGR leak on exit.**
   - Launch fdemon, move the mouse rapidly, press Q.
   - Shell prompt must return cleanly with no `;NN;NNm` / `;NN;NNM` byte
     patterns visible.
   - Repeat with `kill -INT <pid>` (or Ctrl+C if no signal-passthrough
     issue) to confirm the SIGINT path also exits cleanly via the same
     `Message::Quit` route.

## Verification checklist

- [ ] `cargo fmt --all -- --check` — pass
- [ ] `cargo check --workspace --all-targets` — pass
- [ ] `cargo test --workspace` — pass (record test count delta from
  baseline)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — pass
- [ ] Manual smoke: Bug 1, all three modes click
- [ ] Manual smoke: Bug 2, pointer changes on supporting terminal AND no
  garbage on unsupported terminal
- [ ] Manual smoke: Bug 3, clean exit with no SGR leak

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| None | Verification-only task — no source files modified |

### Notable Decisions/Tradeoffs

1. **Verification only**: This task ran the quality gate and recorded results. No code changes were made or needed.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (totals below)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

**Test totals across all crates and test binaries:**

| Test binary | Passed | Failed | Ignored |
|-------------|--------|--------|---------|
| fdemon-app (lib) | 2125 | 0 | 4 |
| fdemon-app (additional) | 372 | 0 | 0 |
| fdemon-tui (lib) | 740 | 0 | 3 |
| fdemon-tui (additional) | 842 | 0 | 0 |
| fdemon-daemon (lib) | 1013 | 0 | 1 |
| fdemon-daemon (additional #1) | 14 | 0 | 0 |
| fdemon-daemon (additional #2) | 16 | 0 | 0 |
| integration tests | 80 | 0 | 62 |
| fdemon-core (lib) | 7 | 0 | 0 |
| fdemon-core (additional) | 103 | 0 | 23 |
| flutter-demon (additional binaries) | 15 | 0 | 12 |
| **Total** | **4327** | **0** | **105** |

### Risks/Limitations

1. **Manual smoke tests not performed**: The automated quality gate (fmt, check, test, clippy) all pass. Manual smoke tests for Bug 1 (mode button clicks), Bug 2 (pointer shape), and Bug 3 (SGR leak on exit) require a running terminal and are marked as unchecked in the verification checklist above — they must be verified by a developer in a live terminal session.
