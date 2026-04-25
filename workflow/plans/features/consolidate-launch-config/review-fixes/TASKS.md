# Task Index — PR #35 Review Fixes (Option α)

Parent plan: [../PLAN.md](../PLAN.md)
Parent tasks: [../TASKS.md](../TASKS.md)

**Approved direction (2026-04-25):** Option α — broaden the startup gate so `settings.local.toml`'s cached `last_device` triggers `StartupAction::AutoStart` even when no `auto_start = true` config is present. This makes Tier 2 of `find_auto_launch_target` reachable, which is the only way Task 02's symmetric persistence delivers the "remember last selection" UX promised by parent PLAN.md §3 and §5. Without this fix, `settings.local.toml`'s `last_device`/`last_config` fields are write-only — Task 02 populates them but no code path reads them.

## Tasks

| # | Task | File | Agent | Status | Depends on |
|---|------|------|-------|--------|------------|
| 05 | Broaden the startup gate to fire `AutoStart` on cached `last_device` presence | [tasks/05-broaden-startup-gate.md](./tasks/05-broaden-startup-gate.md) | implementor | [x] Done (validated PASS, merged) | — |
| 06 | Refactor `try_cached_selection` to log on real validation failure (Copilot #2 fix; dead-code cleanup that also becomes user-visible after Task 05) | [tasks/06-refactor-cached-selection.md](./tasks/06-refactor-cached-selection.md) | implementor | [x] Done (validated PASS, merged) | — |
| 07 | Update docs to describe the widened gate, fix "log buffer" wording (Copilot #3), fix `example/app3/.fdemon/launch.toml` header comment (Copilot #4), uplift CHANGELOG entry | [tasks/07-docs-and-changelog.md](./tasks/07-docs-and-changelog.md) | implementor | [x] Done (validated PASS) | 05, 06 (describe final code state) |

## Wave Plan

- **Wave 1:** Tasks 05 and 06 in parallel (worktree). Different crates, zero write-file overlap.
- **Wave 2:** Task 07 sequentially on the merged branch.

## File Overlap Analysis

### Files Modified (Write) — per task

| Task | Files Modified (Write) | Files Read (dependency) |
|------|------------------------|--------------------------|
| 05 | `crates/fdemon-tui/src/startup.rs` | `crates/fdemon-app/src/config/settings.rs` (uses `load_last_selection` and `LastSelection` — read-only) |
| 06 | `crates/fdemon-app/src/spawn.rs` | `crates/fdemon-app/src/config/settings.rs` (uses `validate_last_selection` — read-only) |
| 07 | `docs/CONFIGURATION.md`, `website/src/pages/docs/configuration.rs`, `example/app3/.fdemon/launch.toml`, `CHANGELOG.md` | all of Wave 1 read-only |

### Overlap Matrix (Wave 1 peers)

|        | 05 | 06 |
|--------|----|----|
| **05** | —  | none (different crates: `fdemon-tui` vs `fdemon-app`) |
| **06** | none | — |

**Strategy:** Wave 1 runs in parallel with worktree isolation. Task 07 follows sequentially after the merge.

**Coordination rule for Wave 1:** Neither task may modify the public API of `crates/fdemon-app/src/config/settings.rs` (`load_last_selection`, `validate_last_selection`, `save_last_selection`, `LastSelection`). Both tasks consume that API as a read-only contract; if either task discovers it needs a signature change, stop and escalate.

## Documentation Updates

### Managed by doc_maintainer
**None.** No module additions/removals/renames, no layer-dependency changes, no new build commands.

### Unmanaged (Task 07)
- `docs/CONFIGURATION.md` — rewrite the gate description in "Auto-Start Behavior", fix "log buffer" wording.
- `website/src/pages/docs/configuration.rs` — same fixes mirrored.
- `example/app3/.fdemon/launch.toml` — fix header comment to reflect that `auto_start = true` is on "Development", not "Profile (Issue #25)".
- `CHANGELOG.md` — uplift the Bug Fixes line so it captures the cache-triggers-auto-launch UX, not just the launch.toml-vs-cache priority fix.

## Verification (run once after Task 07 merges)

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Manual smoke (regression coverage on top of Test J):

1. With `example/app3`, **remove** `auto_start = true` from the "Development" config (so no config has it).
2. Delete `example/app3/.fdemon/settings.local.toml`.
3. `cargo run -- example/app3` → expect the New Session dialog (no cache, no auto_start).
4. Pick iPhone simulator (or any visible device). Session launches. Quit.
5. Verify `example/app3/.fdemon/settings.local.toml` now has `last_device = "<UUID>"`.
6. `cargo run -- example/app3` again — **expect: session auto-launches on the cached device, no dialog.** This is the Option B UX that Tier 2 unreachability had been blocking.
7. Disconnect the cached device. `cargo run -- example/app3` — expect: cascades to Tier 3 (first config + first device), launches on whatever's first, logs a warning to the fdemon tracing log file.

## Out of scope (explicit non-goals)

- **Option β** (cache-stale → show dialog instead of cascade). If users complain about the surprise-device behavior in step 7 above, revisit as a follow-up. Not blocking release.
- **Copilot review comment #5** — raw ESC byte in `tests/e2e/snapshots/e2e__e2e__pty_utils__startup_screen.snap`. Latent bug in `tests/e2e/pty_utils.rs::sanitize_for_snapshot` (regex strips only `\x1b\[...` CSI sequences, not bare `\x1b`). This branch's commit `2828936` accepted a snapshot containing one bare ESC, but the underlying sanitization gap predates this PR. **File a follow-up issue** with the body sketched in §3 of the parent plan; do NOT touch `pty_utils.rs` or re-record snapshots in this wave (E2E re-record risk is too high for a release-prep PR).
- **Headless mode auto-launch parity** — `src/headless/runner.rs::headless_auto_start` ignores `launch.toml` and `settings.local.toml` entirely (always launches on `devices.first()` with no config). This is an unrelated existing inconsistency. Out of scope.

## Risks

1. **User-visible behavior change.** After Task 05 merges, users with an existing `settings.local.toml` (e.g. anyone who ran fdemon on `example/app3` during development) will now get auto-launch on first run instead of the dialog. This is the intended Option B UX, but worth flagging in the CHANGELOG entry (Task 07).
2. **Stale-cache cascade.** When the cached device is gone, Tier 3/4 fires and launches on first device + first config silently. Acceptable for option α; matches existing Priority 1 fall-through semantics. If pain emerges, option β follow-up adds a "stale → dialog" return path.
3. **Helper read of `settings.local.toml` runs twice per startup** (once in the gate, once in `try_cached_selection`). Tiny TOML, microsecond-scale, no contention; not worth optimizing.
