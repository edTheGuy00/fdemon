# Action Items — `cache-auto-launch-gate`

**Review Date:** 2026-04-29
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 4 critical, 2 major
**Source review:** [REVIEW.md](./REVIEW.md)

---

## Critical Issues (Must Fix)

### C1. Migration `info!` fires every startup; spec requires "one-time"
- **Source:** `bug_fix_reviewer`, `code_quality_inspector`, `logic_reasoning_checker`, `risks_tradeoffs_analyzer`
- **Files:**
  - `crates/fdemon-tui/src/startup.rs:57-63`
  - `src/headless/runner.rs:271-281`
- **Problem:** BUG.md §Decisions §5 says "emit a **one-time** `info!` log". Current implementation re-emits on every startup that meets the condition. CI/script users see the same nudge in every log file.
- **Required Action:** Wrap the `tracing::info!` call site in a process-level `OnceLock<()>` guard. Mirror the existing pattern at `crates/fdemon-app/src/config/settings.rs:367` (used for the deprecated `auto_start` warning). If the migration helper from m1 is extracted, host the `OnceLock` in that helper.
- **Acceptance:**
  - [ ] Running `fdemon` twice in succession against a project with cache + no `auto_launch` produces the migration log once per process invocation (not twice within a single process).
  - [ ] Existing G1–G5 tests still pass.

---

### C2. Headless migration nudge advises an action that has no effect in headless
- **Source:** `logic_reasoning_checker`, `risks_tradeoffs_analyzer`
- **File:** `src/headless/runner.rs:275-280`
- **Problem:** Headless hard-wires `cache_allowed = false` regardless of `[behavior] auto_launch`. The current copied-from-TUI message tells users to set `auto_launch = true` to restore previous behavior — but doing so in headless changes nothing.
- **Required Action:** Replace the headless message text with one that reflects headless semantics. Suggested wording:

  > *"settings.local.toml has a cached last_device. Headless mode is intentionally cache-blind — it picks the first available device or honors per-config `auto_start = true` in launch.toml. The `[behavior] auto_launch` flag does NOT apply in headless."*

  Alternatively, suppress the log entirely in headless if no actionable directive remains.
- **Acceptance:**
  - [ ] Headless log message no longer references `[behavior] auto_launch` as a remediation.
  - [ ] The TUI message at `crates/fdemon-tui/src/startup.rs` is unchanged (still references `auto_launch` since it IS effective there).

---

### C3. `find_auto_launch_target` is `pub` but its panic path is undocumented; line-number comment is stale
- **Source:** `bug_fix_reviewer`, `code_quality_inspector` (MAJOR), `logic_reasoning_checker`, `security_reviewer`
- **Files:**
  - `crates/fdemon-app/src/spawn.rs:225` (function declaration)
  - `crates/fdemon-app/src/spawn.rs:330` (panic in `bare_flutter_run`)
- **Problem:** The function was promoted to `pub` in this change and is now called cross-crate from `src/headless/runner.rs:306`. It can reach `bare_flutter_run`'s `.expect("devices non-empty; checked at spawn_auto_launch line 137")` — but the line reference is stale (the actual guard moved as part of this PR), and the public doc comment has no `# Panics` section.
- **Required Action:** Pick **one** of:
  - **(a) Preferred:** Make `bare_flutter_run` return `Option<AutoLaunchSuccess>` and propagate the `None` up. `find_auto_launch_target` becomes `pub fn ... -> Option<AutoLaunchSuccess>` (or returns a `bare_flutter_run` fallback only when devices exist). Removes the panic. Update the two call sites (`spawn_auto_launch`, `headless_auto_start`).
  - **(b) Minimum:** Add a `/// # Panics\n/// Panics if `devices` is empty. Callers must guarantee at least one device.` section to `find_auto_launch_target`'s doc comment, AND replace the stale line reference in the `expect` message with a function-name-based one (e.g., `"non-empty; precondition of find_auto_launch_target"`).
- **Acceptance:**
  - [ ] Either no `expect()` reachable from a `pub` function, OR the public doc clearly documents the panic precondition.
  - [ ] No line numbers appear in panic/expect messages.
  - [ ] Headless and TUI call sites still compile and tests pass.

---

### C4. Sibling-bug coordination is orphaned
- **Source:** `risks_tradeoffs_analyzer`
- **Files:**
  - `src/headless/runner.rs:244-249` (location of absorbed wiring)
  - `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` (sibling plan)
- **Problem:** Task 04 absorbed the sibling bug `launch-toml-device-ignored` Task 03's `find_auto_launch_target` wiring inline. The sibling plan still claims that task as outstanding. When that branch eventually lands, the sibling reviewer will be confused about why their PR seems to do nothing — or worse, they will ship a duplicate path with subtly different `cache_allowed` defaults.
- **Required Action:**
  1. Add a header comment at `src/headless/runner.rs` near `headless_auto_start` noting: *"`find_auto_launch_target` integration here was originally scoped to sibling bug `launch-toml-device-ignored` Task 03; absorbed inline by `cache-auto-launch-gate` Task 04 (option b) on 2026-04-29."*
  2. Update `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` Task 03 status: *"SUPERSEDED — wiring absorbed by `cache-auto-launch-gate` Task 04. Close as resolved without separate implementation."*
- **Acceptance:**
  - [ ] Cross-reference comment exists in `src/headless/runner.rs`.
  - [ ] Sibling TASKS.md row marked superseded with date.

---

## Major Issues (Should Fix)

### M1. Settings Panel toggle gives no "restart required" affordance
- **Source:** `risks_tradeoffs_analyzer`
- **File:** `crates/fdemon-app/src/settings_items.rs:91-95`
- **Problem:** `auto_launch` is read once at startup (`runner.rs:181`); toggling and saving has no effect on the current session. Manual smoke test #6 in TASKS.md says "Restart fdemon" — the team knows, but the UI doesn't.
- **Suggested Action:** Update the Settings Panel description to: *"Auto-launch the cached device on startup (takes effect on next fdemon launch)."*
- **Acceptance:** Description string includes "next fdemon launch" or equivalent restart hint.

---

### M2. Migration nudge is invisible to most users
- **Source:** `risks_tradeoffs_analyzer`
- **Files:** Same as C1.
- **Problem:** `tracing::info!` writes only to a log file most users never inspect. The behavior change is the primary R2 mitigation in BUG.md — but the channel is too quiet.
- **Suggested Action (pick one or two):**
  - Promote to `warn!` (one-line change).
  - Add a one-time TUI banner above the New Session dialog when this migration condition fires.
  - At minimum, ensure C1 (one-time gating) is in place so it doesn't become spam.
- **Acceptance:** First post-upgrade run surfaces the change in a way the user is likely to notice (warn level, TUI hint, or equivalent).

---

## Minor Issues (Consider Fixing)

- **m1.** Extract migration-condition + log helper to `fdemon-app::config` to dedupe TUI/headless logic. (architecture_enforcer)
- **m2.** Track tech-debt issue: convert `cache_allowed: bool` to `enum CachePolicy`. (risks_tradeoffs_analyzer)
- **m3.** Add handler-level test: `Message::StartAutoLaunch { cache_allowed: false }` → `UpdateAction { cache_allowed: false }`. (code_quality_inspector)
- **m4.** Add end-to-end integration test threading `Settings::load` → `Engine::new` → `dispatch_startup_action` for the gate. (risks_tradeoffs_analyzer)
- **m5.** Add doc warning to public `has_cached_last_device`: *"Performs sync I/O — do not call from render or hot paths."* (architecture_enforcer)
- **m6.** Pre-existing `warn!` interpolation of `config.device` in `spawn.rs:262/311` — switch to `{:?}` formatting. (security_reviewer)

## Nitpicks

- **n1.** Add inline comment in G4 test explaining what it adds over G3.
- **n2.** Pick `tempdir()` or `tempfile::tempdir()` consistently in `startup.rs` tests.
- **n3.** Capture-based test for migration `info!` emission (acknowledged tracing ergonomics).
- **n4.** Track headless-TEA-bypass cleanup (pre-existing, now extended).

---

## Re-review Checklist

After addressing C1–C4 (and ideally M1, M2):

- [ ] All four critical issues resolved.
- [ ] Quality gate green:
  - `cargo fmt --all -- --check`
  - `cargo check --workspace --all-targets`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual smoke test #1 from BUG.md (cache + no opt-in → dialog) shows the migration log **once** per process.
- [ ] Manual smoke test #4 from BUG.md (headless backwards compat) passes; headless log message is now headless-specific.
- [ ] Sibling bug `launch-toml-device-ignored` TASKS.md updated.

After re-review passes: ✅ APPROVED.
