# Review: Mouse Support — Phase 1 Foundation

**Review Date:** 2026-05-02
**Branch:** `feat/mouse-support`
**Diff Range:** `3396d3891b9c84d2f6fb6f813c7b02cbe30205bd..HEAD` (6 commits)
**Plan:** `workflow/plans/features/mouse-support/phase-1-foundation/TASKS.md` (6 tasks, all marked Done)
**Reviewers:** architecture_enforcer, code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer

## Verdict: ⚠️ NEEDS WORK

**Why not APPROVED:** Phase 1's own success criteria (`cargo clippy --workspace --all-targets -- -D warnings`) are demonstrably not met — there is a documented but unresolved clippy failure that 4 of 5 reviewers flagged. Three other notable gaps need decisions before merge: a missing integration test that the plan explicitly required, a misleading "no behavior change" contract, and zero cross-platform validation for a default-on terminal feature.

**Why not REJECTED:** The structural work is sound. Layer boundaries are clean, the AtomicBool guard against crossterm #613 is correct, the TEA pattern is respected, and the security surface is narrow. All the blocking issues are small fixes or scope-decisions, not redesigns.

## Reviewer Verdicts at a Glance

| Reviewer | Verdict | Key Finding |
|----------|---------|-------------|
| architecture_enforcer | PASS | 1 warning (pre-existing clippy already in TASKS.md) |
| code_quality_inspector | NEEDS WORK | 1 CRITICAL (clippy), 2 MAJOR (missing docs, missing test) |
| logic_reasoning_checker | WARNINGS | Atomic ordering, panic-hook idempotency, `Click` naming |
| risks_tradeoffs_analyzer | CONCERNS | Default-on ergonomics, no Windows/Linux smoke test, "no behavior change" claim is false |
| security_reviewer | PASS | 0 critical / 2 medium / 1 low — all narrow attack surface |

## Summary of What Shipped

**6 tasks merged across 3 waves:**
- 01 — `MouseInput` / `MouseButton` / `ScrollDir` / `KeyModSet` abstract types in `fdemon-app/src/input_mouse.rs`
- 02 — `Message::Mouse(MouseInput)` + `handler::mouse::handle_mouse` no-op shell (exhaustive `UiMode` match, returns `None` for every variant)
- 03 — `[ui] enable_mouse: bool` config setting (default `true`) + settings panel entry + `apply_project_setting` toggle
- 04 — Crossterm `Event::Mouse` → `Message::Mouse(MouseInput)` conversion at `fdemon-tui/src/event.rs` (drops `Moved`, exhaustively maps the rest)
- 05 — `MOUSE_CAPTURE_ON: AtomicBool` + `enable_mouse_capture()` / `disable_mouse_capture()` + panic-hook integration in `fdemon-tui/src/terminal.rs`
- 06 — Wire `enable/disable` calls into `run_with_project` and `run_with_project_and_dap` (gated on `settings.ui.enable_mouse`); demo `run()` intentionally left untouched

Implementation totals: ~850 LOC across 16 source files + 4 snapshot updates, ~30 new unit tests across 4 crates.

## Consolidated Findings (deduplicated)

### 🔴 CRITICAL — must fix before merge

**C1. Three `assertions_on_constants` clippy errors break Phase 1's own success criteria**
[Source: code_quality_inspector, architecture_enforcer, risks_tradeoffs_analyzer, logic_reasoning_checker]
- **File:** `crates/fdemon-app/src/input_mouse.rs:182-184`
- The test `test_keymodset_none_is_empty` calls `assert!(!KeyModSet::NONE.shift)` (and `.ctrl`, `.alt`). Because `NONE` is `pub const`, each assertion reduces to `assert!(false)` at compile time, which `clippy::assertions_on_constants` rejects under `-D warnings`.
- TASKS.md line 44 already logs this as a pre-merge concern; it has not been fixed.
- **Fix:** Bind to a local `let none = KeyModSet::NONE;` first, then assert on `none.shift` etc. (Avoids `#[allow]` which would mask future regressions on actually-runtime values that happen to be constant.)

### 🟠 MAJOR — should fix before merge

**M1. Missing integration test for `update(state, Message::Mouse(...))`**
[Source: code_quality_inspector, logic_reasoning_checker]
- TASKS.md line 87 explicitly lists this as a success criterion ("`update(state, Message::Mouse(...))` returns `UpdateResult::none()` and does not mutate state").
- The 22 `handle_mouse` no-op tests cover the inner function, but the outer `update()` routing in `handler/update.rs:60-66` has no test — a regression that wires `Message::Mouse` to a side effect would silently pass CI.
- **Fix:** Add one test in `crates/fdemon-app/src/handler/tests.rs` that builds an `AppState`, snapshots `state.ui_mode`, calls `update(&mut state, Message::Mouse(Click { ... }))`, and asserts both `result.message.is_none() && result.action.is_none()` and `state.ui_mode` is unchanged.

**M2. Two public functions in `event.rs` have no doc comments**
[Source: code_quality_inspector]
- `pub fn key_event_to_input` and `pub fn poll` in `crates/fdemon-tui/src/event.rs` are public API but lack `///` docs. This violates `docs/CODE_STANDARDS.md` ("All `pub` functions and types must have `///` doc comments").
- **Fix:** Add `///` headers describing purpose, return semantics (especially that `Some(None)` paths are filtered), and any caller obligations.

**M3. "No behavior change" claim in TASKS.md is false in practice**
[Source: risks_tradeoffs_analyzer, security_reviewer]
- TASKS.md line 7: "a user can scroll/click anywhere in fdemon and nothing changes." This is incorrect — enabling mouse capture sends DECSET 1000/1002/1003/1015/1006, which silently breaks native terminal text-selection (without Shift) and intercepts wheel scroll that previously moved the host scrollback. These ARE user-visible changes even before any handler does work.
- **Fix:** Either (a) reword the contract to "no fdemon TEA-state change; user-visible terminal behavior is intentionally altered" and document the trade-off, or (b) defer the default-on flip to Phase 2 when there's a benefit to compensate. See R1 below.

**M4. No cross-platform smoke test for a default-on terminal feature**
[Source: risks_tradeoffs_analyzer]
- TASKS.md success criteria only cover macOS manual smoke test. The Windows risk profile is non-trivial:
  - Crossterm #613 (legacy conhost panic on disable-without-enable) — guarded by AtomicBool, but the guard is only test-asserted, never verified on real Windows.
  - Crossterm #986 (Shift-modifier-on-mouse broken on Win11) — dormant in Phase 1 but ships now.
  - Legacy conhost (Win10 default pre-Terminal app) silently no-ops `EnableMouseCapture`, returning `Ok(())` — feature appears broken with no error.
- **Fix:** Run the manual smoke test on Windows (both Windows Terminal and legacy conhost if possible) and Linux (gnome-terminal + tmux) before sign-off. Document any platform degradation in `docs/CONFIGURATION.md`.

### 🟡 MINOR — fix soon

**N1. `Ordering::SeqCst` on `MOUSE_CAPTURE_ON` is overly strong**
[Source: code_quality_inspector, logic_reasoning_checker, security_reviewer]
- `crates/fdemon-tui/src/terminal.rs` lines 58, 72, 91, 103, 114, 118, 128 all use `SeqCst`. The semantics needed are a single happens-before pair: `Release` on `enable`'s store, `Acquire` on `disable`'s swap. `SeqCst` adds a global total order across all atomics, which is unnecessary overhead on weakly-ordered architectures (ARM).
- **Fix:** Use `Release` for the store, `Acquire` (or `AcqRel` on the swap) for the load/swap. Tests that reset the flag can use `Relaxed`.

**N2. `install_panic_hook()` is not idempotent**
[Source: logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer]
- Each call wraps the current hook with mouse-disable logic. If both `run_with_project` and `run_with_project_and_dap` were ever called in the same process, the chain would contain two mouse-disables. Not a current bug (mutually exclusive entry points), but invites future drift.
- **Fix:** Add a `static HOOK_INSTALLED: AtomicBool` guard; mirror the existing pattern in the same file.

**N3. `MouseInput::Click` is misleadingly named — it's emitted on `Down`**
[Source: logic_reasoning_checker]
- Crossterm distinguishes `Down` (button pressed) from a debounced click. The variant `MouseInput::Click` is emitted on `MouseEventKind::Down`, which is press semantics. Phase 2 implementers will likely assume click-debounce.
- **Fix:** Either rename to `Press` now (cheap; one pre-public consumer) or add a prominent doc-comment on the variant.

**N4. Default `enable_mouse: true` has no in-app discoverability of off-switch**
[Source: risks_tradeoffs_analyzer]
- A user surprised by broken text selection has to know to open Settings → UI → "Mouse Support" → toggle → restart. No first-launch hint, no footer message.
- **Fix:** Add a one-time hint in the help/footer ("Mouse on — Shift+drag for native selection"), gated on a settings flag like `ui.mouse_hint_seen`. Or flip default to `false` for Phase 1 and revisit in Phase 2 (see R1).

**N5. `[ui] enable_mouse` is missing from `docs/CONFIGURATION.md`**
[Source: doc_freshness]
- The `[ui]` settings table in `docs/CONFIGURATION.md` (lines 308-326) does not list `enable_mouse`. User-facing setting must be documented.
- **Fix:** Add a row to the `[ui]` table and an entry in the example block. Recommend dispatching `doc_maintainer`.

### 🔵 NITPICKS — consider

**P1. `crates/fdemon-tui/src/event.rs:1` module doc says only "Terminal event polling"** — module now also handles key conversion + mouse conversion; expand the `//!` header. [Source: code_quality_inspector]

**P2. Forward-compat: `KeyModSet` lacks a `cmd` field; `Scroll` lacks a `lines` delta**
[Source: risks_tradeoffs_analyzer]
- macOS Cmd-click and trackpad fractional/multi-line scroll would force breaking-API changes if added later. Cheap to add now (additive), expensive after Phase 2 consumers exist.

**P3. DECSET 1003 (any-motion) is enabled but `Moved` is dropped at the boundary** — high-frequency events still cross the PTY for nothing. Consider documenting the trade-off, or downgrading to `?1002h` (button-event only) until drag is needed in a later phase. [Source: security_reviewer, risks_tradeoffs_analyzer]

**P4. Panic-hook ordering invariant is undocumented**
[Source: logic_reasoning_checker]
- `disable_mouse_capture()` is called before `ratatui::restore()`. This works because DECRST mouse modes are connection-global, not alt-screen-scoped. A one-line comment in `terminal.rs::install_panic_hook` would prevent a future ratatui upgrade from silently inverting this assumption.

**P5. `assert_eq!(items.len(), 35)` settings count test is brittle**
[Source: risks_tradeoffs_analyzer]
- Bumped from 34 to 35 in Task 06; will keep churning every time anyone adds a setting. Consider replacing with a per-id existence check (`items.iter().any(|i| i.id == "ui.enable_mouse")`).

**P6. Insta snapshots embed the crate version string**
[Source: risks_tradeoffs_analyzer]
- Task 06 had to update 4 snapshots from v0.4.2 → v0.4.3 — recurring per-release tax. Configure insta with `[tool.insta]` redactions or `with_settings!{ filters }` to strip the version line.

## Risk & Trade-off Summary

| Decision | Verdict | Notes |
|----------|---------|-------|
| `MouseInput` / `KeyModSet` in fdemon-app, no crossterm dep | Sound | Layer purity preserved; mirrors `InputKey` |
| `Message::Mouse` per event with no-op handler | Sound | Per-event cost negligible at click rates; symmetric with `Message::Key` |
| `AtomicBool` over `Mutex<bool>` / `OnceCell` | Sound | Lock-free, panic-safe, std-only |
| Skip `selector.rs` and demo `run()` | Sound | Documented; selector predates settings |
| Default `enable_mouse: true` in Phase 1 | **Questionable** | Ships behavior change with no Phase 1 user-visible benefit; see M3 + N4 |
| Adjacent fixes in Task 06 (snapshot + count test) | Mixed | Necessary to satisfy AC6/AC7, but masks brittle test patterns; see P5 + P6 |
| Manual macOS-only smoke test | **Not acceptable** | See M4 |

## Documentation Freshness

- ⚠️ `docs/CONFIGURATION.md` — needs `[ui] enable_mouse` row (see N5)
- ✅ `docs/ARCHITECTURE.md` — module tables are sparse summaries already; new `input_mouse.rs` and `handler/mouse.rs` could be added but not strictly required
- ✅ `docs/CODE_STANDARDS.md` — `Message` enum example is illustrative, not exhaustive; safe to leave
- ✅ `docs/REVIEW_FOCUS.md` — TEA exception policy unchanged (no new `Cell` introduced)

## Re-review Checklist

After addressing issues, the following must pass before re-approval:

- [ ] **C1** Clippy fix in `input_mouse.rs:182-184` — verify `cargo clippy --workspace --all-targets -- -D warnings` is clean
- [ ] **M1** New `update(Message::Mouse)` integration test in `handler/tests.rs`
- [ ] **M2** Doc comments on `pub fn key_event_to_input` and `pub fn poll`
- [ ] **M3** TASKS.md "no behavior change" claim corrected, OR default flipped to `false`
- [ ] **M4** Manual smoke test executed on Windows + Linux; results recorded
- [ ] Decide on each MINOR finding (fix vs. defer with tracking)
- [ ] Standard quality gate: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`

See `ACTION_ITEMS.md` for the actionable punch list.
