# Bugfix Plan: Address review findings from `cache-auto-launch-gate`

**Status:** Approved — ready for orchestration
**Owner:** ed
**Date:** 2026-04-29
**Parent plan:** [`../cache-auto-launch-gate/BUG.md`](../cache-auto-launch-gate/BUG.md)
**Source review:** [`../../../reviews/bugs/cache-auto-launch-gate/REVIEW.md`](../../../reviews/bugs/cache-auto-launch-gate/REVIEW.md) · [`ACTION_ITEMS.md`](../../../reviews/bugs/cache-auto-launch-gate/ACTION_ITEMS.md)

---

## TL;DR

The parent bug fix `cache-auto-launch-gate` shipped successfully but accumulated four critical and two major findings during the multi-agent code review. None require restructuring; all are local fixes on the same branch (`plan/cache-auto-launch-gate`). This plan packages those follow-ups so the parent fix can ship cleanly.

---

## Findings Summary (from code review)

| # | Finding | Severity | Resolved by |
|---|---------|----------|-------------|
| C1 | Migration `info!` fires on every startup; spec required "one-time" | Critical | Task 01 |
| C2 | Headless migration nudge tells users to set a flag that has no effect in headless | Critical | Task 01 |
| C3 | `find_auto_launch_target` is `pub` and reaches `bare_flutter_run`'s `expect()` panic with stale line-number comment | Critical | Task 02 |
| C4 | Sibling-bug `launch-toml-device-ignored` Task 03 was absorbed inline; no marker for future reviewer | Critical | Task 03 |
| M1 | Settings Panel `auto_launch` toggle has no "takes effect on next launch" hint | Major | Task 03 |
| M2 | Migration nudge is `tracing::info!` (file-only) — invisible to most users | Major | Task 04 |

Minor (m1–m6) and nitpick (n1–n4) findings from the review are **explicitly out of scope** for this plan and tracked in [Out of Scope](#out-of-scope) below.

---

## Decisions Locked In

1. **C1 implementation:** Wrap migration log emission in a process-level `OnceLock<()>` guard, mirroring the pattern at `crates/fdemon-app/src/config/settings.rs:367` (`check_deprecated_auto_start`). Persistent (cross-process) dedupe via a `settings.local.toml` sentinel is **not** in scope — process-level dedupe is sufficient and matches the existing in-tree convention.

2. **C1 / C2 refactor:** Extract the migration condition + log emission into a shared helper in `crates/fdemon-app/src/config/mod.rs`. Both TUI and headless call sites delegate to the helper. The helper returns `bool` indicating whether the nudge applied this process, so callers can drive secondary UI (e.g., the TUI banner from Task 04). This bonus dedupe addresses the `architecture_enforcer` suggestion (m1 in the review) at no extra cost.

3. **C2 message divergence:** The shared helper accepts a `mode` parameter (`Tui` / `Headless`) and emits one of two distinct message strings:
   - **TUI:** *"settings.local.toml has a cached last_device but [behavior] auto_launch is not set in config.toml. Auto-launch via cache is now opt-in. Set `[behavior] auto_launch = true` to restore the previous behavior."*
   - **Headless:** *"settings.local.toml has a cached last_device. Headless mode is intentionally cache-blind — it picks the first available device or honors per-config `auto_start = true` in launch.toml. The `[behavior] auto_launch` flag does NOT apply in headless."*

4. **C3 fix flavor (option a):** `bare_flutter_run` returns `Option<AutoLaunchSuccess>` (returns `None` on empty `devices`). `find_auto_launch_target` returns `Option<AutoLaunchSuccess>` accordingly. The two callers (`spawn_auto_launch`, `headless_auto_start`) handle the `None` branch — both already guard `devices.is_empty()` before calling, so the `None` branch is unreachable today, but the type signature now enforces the precondition rather than relying on `expect()`. Eliminates the panic entirely. Replaces option (b) "just add `# Panics` doc" — preferred per user decision.

5. **C4 supersede:** Add a header comment in `src/headless/runner.rs` near `headless_auto_start` documenting the absorbed wiring. Update the sibling plan's `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` Task 03 row to `SUPERSEDED` with a back-reference to this plan. Confirmed acceptable to modify the sibling plan file.

6. **M1 description:** Update the Settings Panel description from *"Auto-launch the last-used device on startup (skipped if launch.toml has auto_start)"* to *"Auto-launch the last-used device on startup (takes effect on next fdemon launch; skipped if launch.toml has auto_start)"*. Single-string edit in `crates/fdemon-app/src/settings_items.rs`.

7. **M2 visibility (both promotions):**
   - Promote the migration log from `tracing::info!` → `tracing::warn!` for higher default-subscriber visibility.
   - Add a one-line banner above the New Session dialog in TUI mode when the migration nudge applied this process. Banner copy: *"⚠ Cache-driven auto-launch is now opt-in. Set `[behavior] auto_launch = true` in `.fdemon/config.toml` to restore."* (No emoji preference — banner text is illustrative; final wording at implementor discretion.)
   - Banner shows only on the first dialog appearance per process. Once the user dismisses or interacts with the dialog, the banner does not re-show.

---

## Affected Code Map

| File | Change | Task |
|------|--------|------|
| `crates/fdemon-app/src/config/mod.rs` | Add `pub fn migration_nudge_applies(...) -> bool` and `pub fn emit_migration_nudge(mode: NudgeMode) -> bool` (with `OnceLock` guard) | 01 |
| `crates/fdemon-app/src/config/mod.rs` | Bump emit level from `info!` → `warn!` (post-Task 01) | 04 |
| `crates/fdemon-tui/src/startup.rs` | Replace inline `tracing::info!` with helper call; capture `bool` to set banner state | 01, 04 |
| `src/headless/runner.rs` | Replace inline `tracing::info!` with helper call (Headless mode) | 01 |
| `src/headless/runner.rs` | Add header comment referencing absorbed sibling-bug Task 03 | 01 |
| `src/headless/runner.rs` | Adjust call site of `find_auto_launch_target` for new `Option` return | 02 |
| `crates/fdemon-app/src/spawn.rs` | `bare_flutter_run` → `Option<AutoLaunchSuccess>`; `find_auto_launch_target` → `Option<AutoLaunchSuccess>`; remove `expect()`; update doc comment | 02 |
| `crates/fdemon-app/src/spawn.rs` | Update `spawn_auto_launch` call site of `find_auto_launch_target` (handle `None`) | 02 |
| `crates/fdemon-app/src/state.rs` (or equivalent) | Add `pub show_migration_banner: bool` to `AppState` | 04 |
| `crates/fdemon-tui/src/widgets/new_session_dialog/...` | Render banner above dialog when flag is set | 04 |
| `crates/fdemon-app/src/settings_items.rs` | Update `behavior.auto_launch` description string | 03 |
| `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` | Mark Task 03 `SUPERSEDED` | 03 |

---

## Out of Scope

These review findings are **deliberately deferred**. They will be tracked separately, not in this plan:

- **m1 (DRY migration helper):** Solved as a byproduct of Task 01 — not a separate task.
- **m2 (`cache_allowed: bool` → enum):** Tech-debt cleanup, no behavioral change. Defer.
- **m3 (handler-level test for `cache_allowed: false` propagation):** Coverage gap, not a regression risk. Defer.
- **m4 (end-to-end integration test of pipeline):** Coverage gap. Defer.
- **m5 (sync-I/O warning doc on `has_cached_last_device`):** Cosmetic. Defer.
- **m6 (`{:?}` formatting for `config.device` in pre-existing `warn!`):** Pre-existing security-MEDIUM, not introduced by this plan. Defer to a separate hardening pass.
- **n1–n4 (test naming clarification, import inconsistency, tracing capture, headless TEA bypass):** All cosmetic or pre-existing. Defer.

If any of the deferred items become blocking later, file a new bug plan.

---

## Verification

After all four tasks merge:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Manual smoke tests (specific to these followups)

1. **C1 (one-time per process):** In `example/app2` with cache + no `auto_launch`, run `fdemon` → New Session dialog appears + migration `warn!` line in fdemon log. Quit. Re-run `fdemon` → log file gets a **second** `warn!` line (one per process). Within a single process, the line appears only once even if `startup_flutter` were called multiple times (defensive).

2. **C2 (headless message divergence):** Same fixture, run `fdemon --headless` → log line uses the headless-specific text (no reference to `[behavior] auto_launch` as a remediation).

3. **C3 (no panic on empty devices):** Theoretical — both callers guard `is_empty`. To verify the type-level fix: `cargo check` should require both call sites to handle `Option`.

4. **C4 (sibling-bug supersede):** `git grep -n "absorbed by cache-auto-launch-gate" src/` returns the new comment. `cat workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` shows the SUPERSEDED status on Task 03's row.

5. **M1 (settings panel hint):** Open `fdemon` Settings Panel (`S`) → Behavior tab → `Auto-launch on cached device` row description ends with `(takes effect on next fdemon launch; …)`.

6. **M2 (visibility):**
   - **warn promotion:** Migration log line is at WARN level (visible to default subscribers).
   - **TUI banner:** First post-upgrade run with cache + no `auto_launch` shows a one-line banner above the New Session dialog with the migration message.

---

## Risks & Mitigations

- **R1 — Banner state lifetime:** The TUI banner needs to clear after dismissal. *Mitigation:* Banner state is a `bool` on `AppState` set by `startup_flutter`; cleared when the user closes the New Session dialog OR when the dialog is replaced by another UI mode. Implementor decides the exact clear-trigger; Task 04 acceptance criteria require *some* clear path so the banner doesn't persist forever.
- **R2 — `Option<AutoLaunchSuccess>` ripples:** Both call sites already guard empty-devices, but the new `None` branch in `headless_auto_start` and `spawn_auto_launch` must produce a sensible behavior (likely: log + return early — same as the pre-fix empty-devices path). *Mitigation:* Task 02 acceptance includes a unit test for the `None` branch.
- **R3 — `OnceLock` test isolation:** Process-level `OnceLock` survives across tests in the same binary. *Mitigation:* Task 01 wraps the helper so test code can use a separate code path or reset path; alternatively, gate the `OnceLock` behind a `#[cfg(test)]` feature flag that's disabled in unit tests. Implementor's call.
- **R4 — Wave 2 dependency on Task 01's helper:** If Task 01 doesn't merge cleanly, Tasks 02 and 04 are blocked. *Mitigation:* Tasks 02 and 04 are independent of Task 01's specific helper signature for their core functionality (Task 02 only needs `spawn.rs` and `headless/runner.rs`; Task 04 needs the helper's `bool` return). If Task 01 is delayed, Task 02 can still proceed in isolation; Task 04 must wait.
