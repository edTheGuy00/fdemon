# Task 01 — Migration nudge helper, `OnceLock` gate, headless message divergence

**Plan:** [../BUG.md](../BUG.md) · **Index:** [../TASKS.md](../TASKS.md)
**Agent:** implementor
**Depends on:** —
**Wave:** 1 (parallel with Task 03)

## Goal

Resolve review findings **C1** (migration `info!` fires every startup, spec required "one-time"), **C2** (headless message text gives misleading remediation advice), and the *header-comment half* of **C4** (sibling-bug coordination undocumented in code).

Three concerns share the same code sites (the migration log block in `crates/fdemon-tui/src/startup.rs:57-63` and `src/headless/runner.rs:271-281`), so they're solved together by:

1. Extracting the migration-condition + log emission into a shared helper in `crates/fdemon-app/src/config/mod.rs`.
2. Wrapping the helper's emission in a process-level `OnceLock<()>` guard (mirrors `crates/fdemon-app/src/config/settings.rs:367` `check_deprecated_auto_start`).
3. Emitting different message text based on a `mode: NudgeMode` parameter (`Tui` vs `Headless`).
4. Returning a `bool` from the helper indicating whether the nudge applied this process (consumed by Task 04's TUI banner; ignored by headless).
5. Adding a header comment in `src/headless/runner.rs` documenting that `find_auto_launch_target` was absorbed from sibling-bug `launch-toml-device-ignored` Task 03.

**No log-level change in this task.** This task keeps `tracing::info!`. Task 04 promotes it to `tracing::warn!`.

## Files Modified (Write)

| File | Change |
|------|--------|
| `crates/fdemon-app/src/config/mod.rs` | (1) Add `pub enum NudgeMode { Tui, Headless }`. (2) Add `pub fn emit_migration_nudge(mode: NudgeMode, project_path: &Path, settings: &Settings) -> bool` with `OnceLock` guard around the actual `tracing::info!` emission. Returns `true` if the condition applied this process (regardless of whether the `OnceLock` already fired — so callers can drive secondary UI like Task 04's banner consistently across calls). (3) Inside the function, compute `has_auto_start_config = get_first_auto_start(&load_all_configs(project_path)).is_some()`, `has_cache = has_cached_last_device(project_path)`, `cache_opt_in = settings.behavior.auto_launch`. Emit (gated by `OnceLock`) the appropriate message text per `mode`. |
| `crates/fdemon-tui/src/startup.rs` | Replace inline migration `tracing::info!` block (lines 55-63) with `let _migration_applied = emit_migration_nudge(NudgeMode::Tui, project_path, settings);`. Drop the `_` prefix only if Task 04 lands in the same release; for now, capture as `_migration_applied` so Task 04 can promote it to `state.show_migration_banner = migration_applied`. |
| `src/headless/runner.rs` | (1) Replace inline migration `tracing::info!` block (lines 268-281) with `let _ = emit_migration_nudge(NudgeMode::Headless, &project_path, &engine.settings);`. (2) Add a header comment block immediately above `headless_auto_start` (line ~244) documenting the absorbed sibling-bug wiring. |

## Files Read (dependency)

- `crates/fdemon-app/src/config/settings.rs` (read the `check_deprecated_auto_start` `OnceLock` pattern at line 367 and replicate it)

## Implementation Notes

### Helper signature and message strings

```rust
// In crates/fdemon-app/src/config/mod.rs

/// Identifies the calling context so the migration nudge can produce
/// mode-appropriate remediation text.
pub enum NudgeMode {
    Tui,
    Headless,
}

/// Emit a one-time-per-process migration nudge if a cached `last_device`
/// is present but the user has not opted into cache-driven auto-launch.
///
/// Returns `true` if the nudge condition applies (cache present, no
/// auto_start config, `auto_launch` flag unset) — useful for callers
/// that want to drive secondary UI (e.g., a TUI banner). The actual
/// `tracing::info!` emission is gated by a process-level `OnceLock`,
/// so the log line appears at most once per process.
///
/// The returned `bool` reflects the condition itself, not whether
/// the `OnceLock` fired — i.e., this returns `true` on every call
/// when conditions are met, so callers can render UI consistently
/// even if the log was already emitted earlier this process.
pub fn emit_migration_nudge(
    mode: NudgeMode,
    project_path: &Path,
    settings: &Settings,
) -> bool {
    use std::sync::OnceLock;
    static EMITTED: OnceLock<()> = OnceLock::new();

    let configs = load_all_configs(project_path);
    let has_auto_start_config = get_first_auto_start(&configs).is_some();
    let has_cache = has_cached_last_device(project_path);
    let cache_opt_in = settings.behavior.auto_launch;

    let applies = !has_auto_start_config && has_cache && !cache_opt_in;
    if !applies {
        return false;
    }

    EMITTED.get_or_init(|| match mode {
        NudgeMode::Tui => tracing::info!(
            "settings.local.toml has a cached last_device but [behavior] auto_launch \
             is not set in config.toml. Auto-launch via cache is now opt-in. \
             Set `[behavior] auto_launch = true` to restore the previous behavior."
        ),
        NudgeMode::Headless => tracing::info!(
            "settings.local.toml has a cached last_device. Headless mode is intentionally \
             cache-blind — it picks the first available device or honors per-config \
             `auto_start = true` in launch.toml. The `[behavior] auto_launch` flag \
             does NOT apply in headless."
        ),
    });

    true
}
```

> Note: `get_first_auto_start` is currently in `crates/fdemon-tui/src/startup.rs` and `crates/fdemon-app/src/spawn.rs`. Use whichever is already accessible from `fdemon-app::config`. If neither is — verify via `cargo check` — duplicate the `is_some()` check inline rather than introducing a new public function. The condition is one line; DRY is not worth a new public symbol.

### Header comment in `headless/runner.rs`

Immediately above `async fn headless_auto_start(engine: &mut Engine) {` at line ~250, add:

```rust
/// **Sibling-bug coordination note (added 2026-04-29):**
/// The `find_auto_launch_target` integration in this function was originally
/// scoped to sibling bug `launch-toml-device-ignored` Task 03. It was absorbed
/// inline by `cache-auto-launch-gate` Task 04 (option b) on 2026-04-29 because
/// the sibling task had not been implemented anywhere. When the sibling bug's
/// Task 03 is reviewed next, close it as resolved-by-absorption. See:
/// - workflow/plans/bugs/cache-auto-launch-gate/tasks/04-headless-gate.md
/// - workflow/plans/bugs/launch-toml-device-ignored/TASKS.md (Task 03)
```

### Tests

- Add a unit test in `crates/fdemon-app/src/config/mod.rs` (or its `tests.rs`) that calls `emit_migration_nudge` with a tempdir set up to satisfy the condition and asserts `true` is returned. Note: testing the `OnceLock`-based emission across multiple calls within the same test binary is fragile; test the **return value** (which is unconditional) rather than the **log emission**. Document this in the test body comment.
- Update existing tests in `crates/fdemon-tui/src/startup.rs` (G1–G5) only if they break. The startup_flutter call signature is unchanged; only its internals shift. Most tests should pass without modification.
- Add a unit test asserting `NudgeMode::Headless` produces a different log message than `NudgeMode::Tui`. Since the messages are emitted via `tracing::info!` (hard to capture), this can be a structural test: assert via code inspection / a `match` arm count, OR skip the message-content test entirely. Implementor's call.

### Tracing assertions are not required

The CODE_STANDARDS.md and review acknowledge that `tracing::info!` content is hard to assert in unit tests. Do **not** introduce a tracing-test dependency for this. The `bool` return value is the testable contract.

## Verification

- `cargo check -p fdemon-app -p fdemon-tui`
- `cargo check --workspace --all-targets` (binary crate compiles after `headless/runner.rs` changes)
- `cargo test -p fdemon-app config`
- `cargo test -p fdemon-tui startup`
- `cargo clippy --workspace --all-targets -- -D warnings`
- Manual smoke: in `example/app2` with cache + no `auto_launch`, run `fdemon` twice; verify migration log appears in both invocations' log files but only once per process.

## Acceptance

- [ ] `pub fn emit_migration_nudge(mode: NudgeMode, project_path: &Path, settings: &Settings) -> bool` exists in `crates/fdemon-app/src/config/mod.rs`.
- [ ] `pub enum NudgeMode { Tui, Headless }` exists in the same module.
- [ ] Helper uses a `OnceLock<()>` to gate the `tracing::info!` emission process-wide.
- [ ] Headless message text differs from TUI text and **does not** reference `[behavior] auto_launch` as a remediation.
- [ ] `crates/fdemon-tui/src/startup.rs` calls the helper; the inline `tracing::info!` block is gone.
- [ ] `src/headless/runner.rs` calls the helper; the inline `tracing::info!` block is gone.
- [ ] `src/headless/runner.rs` has the sibling-bug coordination header comment near `headless_auto_start`.
- [ ] Existing G1–G5 tests in `crates/fdemon-tui/src/startup.rs` still pass.
- [ ] All workspace tests pass; `cargo clippy` clean.
- [ ] Smoke test: running `fdemon` twice in the same process (e.g., via a test harness) emits the log only once. (If process-restart-required, the log appears once per restart — also acceptable per the spec.)

---

## Completion Summary

**Status:** Done
**Branch:** plan/cache-auto-launch-gate

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/mod.rs` | Added `pub enum NudgeMode { Tui, Headless }` and `pub fn emit_migration_nudge(mode, project_path, settings) -> bool` with `OnceLock` gate. Added 4 unit tests covering true/false return value conditions. |
| `crates/fdemon-tui/src/startup.rs` | Added `emit_migration_nudge` and `NudgeMode` to imports. Replaced inline `tracing::info!` block with `let _migration_applied = emit_migration_nudge(NudgeMode::Tui, project_path, settings);`. |
| `src/headless/runner.rs` | Replaced inline `tracing::info!` block (and local variable computation) with `let _ = emit_migration_nudge(NudgeMode::Headless, &project_path, &engine.settings);`. Added sibling-bug coordination header comment above `headless_auto_start`. Updated imports to remove `get_first_auto_start` and `has_cached_last_device` (now internal to helper). |

### Notable Decisions/Tradeoffs

1. **Local variables preserved in startup.rs**: `has_auto_start_config`, `has_cache`, `cache_opt_in`, and `cache_trigger` are still needed for the startup gate logic. The helper independently re-computes them — this is intentional (the helper is self-contained). The condition is simple and the duplication is minimal (4 lines).

2. **Tests target return value, not log emission**: Per task spec and CODE_STANDARDS.md, `tracing::info!` emission is not asserted. Tests validate the `bool` return value contract. The `OnceLock` being static means log-emission tests would be fragile across test binary invocations — this limitation is documented in test body comments.

3. **Headless message text**: Explicitly avoids referencing `[behavior] auto_launch` as a remediation, since that flag does not apply in headless mode (C2 finding). The TUI message still references it as the opt-in mechanism.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app config` - Passed (519 tests)
- `cargo test -p fdemon-tui startup` - Passed (12 tests, all G1–G5 pass)
- `cargo test --workspace` - Passed (0 failures across all crates)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **OnceLock static in test binary**: The `EMITTED` OnceLock in `emit_migration_nudge` is process-global. If tests within the same binary call `emit_migration_nudge` with conditions satisfied, only the first invocation emits the log. The return value tests avoid relying on emission order. The smoke test (running fdemon twice) would require two separate process invocations to verify — acceptable per spec.
