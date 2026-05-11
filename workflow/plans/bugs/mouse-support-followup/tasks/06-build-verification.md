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
