# Code Review — `cache-auto-launch-gate`

**Date:** 2026-04-29
**Branch:** `plan/cache-auto-launch-gate`
**Diff base:** `cd016bd` (plan-only commit) → `HEAD` (`1c6276d`)
**Plan:** [`workflow/plans/bugs/cache-auto-launch-gate/BUG.md`](../../../plans/bugs/cache-auto-launch-gate/BUG.md)
**Tasks:** [`workflow/plans/bugs/cache-auto-launch-gate/TASKS.md`](../../../plans/bugs/cache-auto-launch-gate/TASKS.md) (6/6 ✅ Done)
**Reviewers:** `bug_fix_reviewer`, `architecture_enforcer`, `code_quality_inspector`, `logic_reasoning_checker`, `risks_tradeoffs_analyzer`, `security_reviewer`

---

## Verdict: ⚠️ NEEDS WORK

The fix correctly re-gates cache-driven auto-launch behind `[behavior] auto_launch` and threads `cache_allowed: bool` through the TEA pipeline cleanly. Layer boundaries are respected; G1–G5 / T6–T7 / H1–H3 tests cover the new gate logic. **However**, two issues directly contradict BUG.md spec or task acceptance:

1. The migration `info!` fires on **every** startup despite BUG.md §Decisions §5 requiring it to be **one-time**.
2. The headless migration nudge text directs users to `[behavior] auto_launch = true`, but headless intentionally ignores that flag — actively misleading.

Plus one MAJOR code-quality concern (`bare_flutter_run` panics with a stale line-number comment in a now-`pub`-reachable code path) and several MINOR / NITPICK items.

These are correctable in a follow-up commit on this branch — none require restructuring. Recommend addressing the four **Critical** items in [ACTION_ITEMS.md](./ACTION_ITEMS.md) before merging.

---

## Per-Agent Verdicts

| Agent | Verdict | Headline finding |
|-------|---------|------------------|
| `bug_fix_reviewer` | ✅ APPROVED | All 6 task acceptance criteria met; W1 stale `expect` comment; W2 migration log "one-time" spec drift |
| `architecture_enforcer` | ✅ PASS | No layer violations; `has_cached_last_device` move to `fdemon-app::config` is correct direction; pre-existing TEA bypass in headless extended (not new) |
| `code_quality_inspector` | ⚠️ NEEDS WORK | MAJOR: `find_auto_launch_target` is now `pub` but reaches `bare_flutter_run` `expect()` without `# Panics` doc; migration log not actually one-time; minor doc gaps |
| `logic_reasoning_checker` | ✅ PASS w/ minor | Gate logic and tier cascade verified clean; M1 misleading headless migration text; M2 log fires per-startup |
| `risks_tradeoffs_analyzer` | ⚠️ CONCERNS | 3 HIGH risks: (1) migration log invisible & repeated, (2) headless text misleading, (3) sibling-bug coordination orphaned; 4 MEDIUM follow-ups |
| `security_reviewer` | ✅ PASS | No new attack surface; pre-existing `warn!` interpolating `config.device` in `spawn.rs:262/311` flagged for future hardening |

---

## Consolidated Findings

### 🔴 CRITICAL — Address before merge

#### C1. Migration `info!` fires on every startup, not "one-time"
**Severity:** MAJOR (per `code_quality_inspector`) / HIGH (per `risks_tradeoffs_analyzer`)
**Source:** `bug_fix_reviewer` (W2), `code_quality_inspector` (#2), `logic_reasoning_checker` (M2), `risks_tradeoffs_analyzer` (Risk #2)
**Files:**
- `crates/fdemon-tui/src/startup.rs:57-63`
- `src/headless/runner.rs:271-281`

**Problem.** BUG.md §Decisions §5 explicitly says: "emit a **one-time** `info!` log... the **first time** fdemon runs against a project with a non-empty cached `last_device` *and* no `[behavior] auto_launch` set." Both call sites currently emit unconditionally on every startup that meets the condition, with no `OnceLock` guard or persistent sentinel. Users in CI loops, or anyone who deliberately wants the dialog and doesn't intend to set `auto_launch`, will see the same message in every log file forever.

The codebase already has the right pattern: `crates/fdemon-app/src/config/settings.rs:367` uses `OnceLock<()>` for the deprecated `auto_start` warning. Mirror it.

**Fix.** Wrap the `tracing::info!` call in a process-level `OnceLock<()>` (cheapest), or write a `auto_launch_migration_seen = true` sentinel into `settings.local.toml` after first emit (honors "one-time across processes" but adds a file write).

---

#### C2. Headless migration log gives advice that does nothing in headless
**Severity:** MEDIUM (per `risks_tradeoffs_analyzer`) / MINOR (per `logic_reasoning_checker`)
**Source:** `logic_reasoning_checker` (M1), `risks_tradeoffs_analyzer` (Risk #7)
**File:** `src/headless/runner.rs:275-280`

**Problem.** The headless migration log copies the TUI text verbatim: *"Set `[behavior] auto_launch = true` to restore the previous behavior."* Per BUG.md §Decisions §2(b), headless is **intentionally cache-blind regardless of `[behavior] auto_launch`**. So a CI/script user reads the nudge, sets the flag, restarts headless... and observes no behavior change. The advice is actively wrong for headless mode.

**Fix.** Diverge the headless message text. Suggested: *"settings.local.toml has a cached last_device. Headless mode is intentionally cache-blind — it always picks the first available device or honors a per-config `auto_start = true` in launch.toml. The `[behavior] auto_launch` flag does NOT apply in headless."* Optionally suppress the headless log entirely if the message has no actionable directive.

---

#### C3. `find_auto_launch_target` is now `pub` but reaches an undocumented panic
**Severity:** MAJOR (per `code_quality_inspector`)
**Source:** `bug_fix_reviewer` (W1), `code_quality_inspector` (#1, #3), `logic_reasoning_checker` (N1), `security_reviewer` (LOW finding)
**File:** `crates/fdemon-app/src/spawn.rs:225` (function), `crates/fdemon-app/src/spawn.rs:330` (panic site)

**Problem.** `find_auto_launch_target` has been promoted to `pub` and is now called cross-crate from `src/headless/runner.rs:306`. It reaches `bare_flutter_run`, which contains:

```rust
.expect("devices non-empty; checked at spawn_auto_launch line 137")
```

Two issues:
1. The `expect` message names a stale line number ("137") that no longer matches the actual non-empty guard (now at `spawn_auto_launch:185` after this change). Future drift will further mislead readers.
2. The `pub fn` doc comment lists tiers but has no `# Panics` section. Per `docs/CODE_STANDARDS.md`, public functions that can panic must document the precondition. The headless caller does guard with `if result.devices.is_empty() { return; }` at `runner.rs:297-301`, but a future external caller might not.

**Fix.** Pick one:
- (a) Convert `bare_flutter_run` to return `Option<AutoLaunchSuccess>`, propagate up so `find_auto_launch_target` returns `Option<AutoLaunchSuccess>`. Cleanest. Removes the panic.
- (b) Add a `/// # Panics\n/// Panics if `devices` is empty. Caller must ensure at least one device.` doc section, AND replace the line-number reference with a function-name reference (e.g., `"non-empty; verified by caller per find_auto_launch_target precondition"`).

---

#### C4. Sibling-bug coordination is orphaned
**Severity:** HIGH (per `risks_tradeoffs_analyzer`)
**Source:** `risks_tradeoffs_analyzer` (Risk #3)
**Files:** `src/headless/runner.rs` (Task 04 absorbed wiring), `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` (sibling, untouched)

**Problem.** Task 04 absorbed the sibling-bug `launch-toml-device-ignored` Task 03's `find_auto_launch_target` wiring inline (per user's option-(b) decision). When the sibling bug's Task 03 eventually merges, it will likely produce duplicate or conflicting code paths in `src/headless/runner.rs`. Nothing in this branch flags the absorption to a future reviewer of the sibling bug.

**Fix.** Two cheap edits:
1. Add a comment block at `src/headless/runner.rs:244` (or wherever `headless_auto_start` begins) noting: *"NOTE: `find_auto_launch_target` integration here was originally scoped to sibling bug `launch-toml-device-ignored` Task 03; absorbed inline by `cache-auto-launch-gate` Task 04 (option b). Sibling Task 03 should be closed as resolved-by-absorption when reviewed."*
2. Add a status note to `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` Task 03: *"Status: SUPERSEDED — wiring absorbed by `cache-auto-launch-gate` Task 04 on 2026-04-29. Close as resolved without separate implementation."*

---

### 🟠 MAJOR — Should fix before merge

#### M1. Settings Panel toggle gives no "restart required" affordance
**Source:** `risks_tradeoffs_analyzer` (Risk #5)
**File:** `crates/fdemon-app/src/settings_items.rs:91-95`

`auto_launch` is read once at startup (`runner.rs:181`). Toggling it in the Settings Panel and saving has no effect on the current session. The Settings Panel description should make this explicit.

**Fix.** Update the description string to: *"Auto-launch the cached device on startup (takes effect on next fdemon launch)."*

---

#### M2. Migration log nudge is invisible to most users
**Source:** `risks_tradeoffs_analyzer` (Risk #1)
**Files:** Same as C1.

`tracing::info!` writes only to the file-based logger; many users never look at `~/Library/Logs/fdemon/...` (or the equivalent). For a behavior change that breaks pre-upgrade workflows (R2), that channel is too quiet. The TUI dialog appearing instead of an auto-launch may be confusing without an in-TUI hint.

**Fix options (pick one or two, in priority order):**
- Promote to `warn!` so it appears at higher severity (minor change).
- Add a one-time TUI banner above the New Session dialog when this migration condition fires, e.g. *"Tip: cache-driven auto-launch is now opt-in. Set `[behavior] auto_launch = true` in `.fdemon/config.toml` to restore."*
- Already covered if C1 (one-time gating) is implemented — at least removes the spam.

---

### 🟡 MINOR — Track for follow-up

#### m1. Migration condition duplicated between TUI & headless
**Source:** `architecture_enforcer` (Suggestion)
**Files:** `crates/fdemon-tui/src/startup.rs:57-63`, `src/headless/runner.rs:275-281`

Both sites repeat `!has_auto_start_config && has_cache && !cache_opt_in`. Extract to a shared helper in `fdemon-app::config` (e.g., `fn should_emit_auto_launch_migration_nudge(...) -> bool`) — once C1 wraps these in a `OnceLock` and C2 diverges the text, the helper becomes a natural place to host the message logic.

#### m2. `cache_allowed: bool` is a flag-argument anti-pattern
**Source:** `risks_tradeoffs_analyzer` (Risk #4)

Threading a raw `bool` through 4 layers signals nothing about *why* the cache is disallowed. A future "DAP-driven launch", "MCP-driven launch", or "explicit reload" entry point will need to pick a value blindly. Track as tech debt: convert to `enum CachePolicy { AllowedIfOptedIn, Disallowed }` in a follow-up.

#### m3. No handler-level test for `cache_allowed: false` propagation
**Source:** `code_quality_inspector` (#6)
**File:** `crates/fdemon-app/src/handler/tests.rs`

All handler tests pass `cache_allowed: true`. A test sending `Message::StartAutoLaunch { cache_allowed: false }` and asserting the resulting `UpdateAction::DiscoverDevicesAndAutoLaunch { cache_allowed: false, .. }` would close the propagation hole. The actual gate behavior is covered by `spawn::tests::cache_allowed_false_skips_tier2_falls_to_tier3` etc., so this is a low-priority addition.

#### m4. No end-to-end integration test of the full pipeline
**Source:** `risks_tradeoffs_analyzer` (Risk #8), `bug_fix_reviewer` (testing assessment)

Each layer is unit-tested, but no test stages a real `config.toml` + `settings.local.toml` and verifies the gate decision flows through `Engine::new` → `dispatch_startup_action` → `Message::StartAutoLaunch.cache_allowed`. A future refactor that switches `engine.settings` field reads would compile and pass all unit tests while regressing behavior.

#### m5. `has_cached_last_device` is a public sync-I/O function with no caller-warning doc
**Source:** `architecture_enforcer` (Suggestion), `risks_tradeoffs_analyzer` (Risk #6)
**File:** `crates/fdemon-app/src/config/mod.rs:48-52`

The function reads `settings.local.toml` on every call. Now that it's `pub`, a future caller could invoke it from a render or hot-path context. Add a doc comment: *"Performs sync file I/O — do not call from render loop or hot message-handler paths."*

#### m6. Pre-existing `warn!` in `spawn.rs` interpolates user-controlled `config.device` string (security-MEDIUM)
**Source:** `security_reviewer`
**File:** `crates/fdemon-app/src/spawn.rs:262, 311`

Pre-existing, not introduced by this change, but `spawn.rs` was substantially modified and the area was reviewed. A malicious `launch.toml` could embed ANSI escape sequences in `device` to corrupt log viewers. Cosmetic-only impact (developer-local tool), but worth a one-line fix: use `{:?}` instead of `{}` to escape control characters.

---

### 🔵 NITPICKS

#### n1. G3/G4 tests have nearly identical assertions
`G3` and `G4` both assert `AutoStart` fires when `auto_start = true`; G4 additionally sets `auto_launch = true`. The added value of G4 is testing flag-interaction. Add a comment in G4's body clarifying "verifies `auto_launch` doesn't break Tier 1 precedence." (`code_quality_inspector` #8)

#### n2. `tempfile::tempdir()` vs `tempdir()` import inconsistency in startup.rs tests
Some tests use the qualified path, some use the imported short form. Trivial cleanup. (`code_quality_inspector` #9)

#### n3. No assertion-test for the migration `info!` emission itself
Tracing is awkward to capture in unit tests. Could be addressed with `tracing-test` or a custom subscriber, but BUG.md acknowledged this would be hard. Coverage gap, not bug. (`bug_fix_reviewer` O4, `logic_reasoning_checker` coverage gap)

#### n4. Task 04's headless absorbed wiring extends pre-existing TEA-bypass in headless
Headless `headless_auto_start` was already imperative (not message-loop-driven) before this change. Task 04 added more imperative work (config load, device emit, migration log) inline. Not a regression — pre-existing pattern — but the inconsistency with TUI's TEA discipline is now larger. (`architecture_enforcer` MINOR)

---

## Documentation Freshness Check

✅ Both `docs/ARCHITECTURE.md` (Task 06) and `docs/CONFIGURATION.md` (Task 05) updated as part of the implementation. `docs/CODE_STANDARDS.md` and `docs/DEVELOPMENT.md` unaffected (no new patterns or build steps). No stale-doc finding.

---

## Quality Gate Verification

Run after addressing critical items:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

(All four passed at HEAD `1c6276d` per orchestrator's post-merge verification.)

---

## Summary

The fix is structurally correct and ships a real bug fix that matches the user's spec. Two issues directly conflict with documented decisions in BUG.md (one-time log; headless message text), and one MAJOR code-quality issue (panic in newly-pub function) needs a doc or refactor before this is safe to publish. All four critical items are <100-line follow-up changes on this same branch.

After C1–C4 are addressed: **APPROVED**.

See [ACTION_ITEMS.md](./ACTION_ITEMS.md) for the actionable punch list.
