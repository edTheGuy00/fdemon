# Code Review: Log text-selection / copy bug fix

**Bug:** [BUG.md](../../../plans/bugs/log-text-selection-broken/BUG.md)
**Tasks:** [TASKS.md](../../../plans/bugs/log-text-selection-broken/TASKS.md)
**Diff base:** `9f70709` (planning commit) → `HEAD` (`cde83d1`) plus uncommitted `docs/MOUSE.md`
**Branch:** `plan/log-text-selection-fix`
**Review date:** 2026-05-15
**Reviewers dispatched:** bug_fix_reviewer, architecture_enforcer, code_quality_inspector, logic_reasoning_checker, security_reviewer, risks_tradeoffs_analyzer

## Verdict: ⚠️ NEEDS WORK

The functional core is correct: dropping `?1003` is the right root-cause fix and the TEA decomposition (clipboard service, right-click → copy, Alt+m toggle, runner queue, status badge) is well-structured. However, **two correctness defects, two unresolved doc-quality regressions, and several test gaps** were identified by independent reviewers. Three of the items were already flagged as CONCERNs during per-task validation but were not addressed before merge.

| Reviewer | Verdict | Findings |
|----------|---------|----------|
| bug_fix_reviewer | ⚠️ CONCERNS | Root-cause correct; 2 should-fix items + 1 observation aligned with prior validator findings |
| architecture_enforcer | ⚠️ CONCERNS | 0 critical, 3 warnings (all docs/visibility hygiene). No layer violations. |
| code_quality_inspector | ⚠️ NEEDS WORK | 2 major + 3 minor + 4 nitpick |
| logic_reasoning_checker | ⚠️ CONCERNS | Logic sound on hot paths; one new finding (Shift+Alt+m silently dropped) |
| security_reviewer | ✅ PASS (with notes) | 0 critical / 0 high / 2 medium / 2 low / 2 info |
| risks_tradeoffs_analyzer | ⚠️ CONCERNS | 2 should-block items + 1 near-blocker + 5 debt items |

---

## Critical / Major Findings (Block Merge)

### 1. `MouseCaptureChanged` follow-up uses `try_send` — state can drift permanently
**Source:** bug_fix_reviewer, security_reviewer, risks_tradeoffs_analyzer
**File:** `crates/fdemon-tui/src/runner.rs:331-335`
**Severity:** Major (correctness)

After `set_mouse_capture()` succeeds, the runner calls `engine.msg_sender().try_send(MouseCaptureChanged { active: target })`. On a full channel, the message is dropped with a `warn!` log only. `AppState::mouse_capture_active` is never updated; the badge lies indefinitely; subsequent `Alt+m` presses compute `target = !state.mouse_capture_active` from the stale value, so the next user toggle attempt may be a no-op (the idempotency guard short-circuits on the matching target).

**Why this is wrong:** The BUG.md design doc explicitly says "The runner returns the actual outcome via a follow-up `Message::MouseCaptureChanged(bool)`. The TEA `mouse_capture_active` field reflects observed state, not intent." `try_send` violates that promise.

**Fix options:** (a) `blocking_send().await` from the runner's Tokio context, (b) on `try_send` failure, write `state.mouse_capture_active = target` directly (the runner already mutates `state.toasts` two lines below).

### 2. Silent `MemoryClipboard` runtime fallback hides clipboard unavailability
**Source:** security_reviewer (MEDIUM), risks_tradeoffs_analyzer (HIGH/BLOCKING)
**File:** `crates/fdemon-tui/src/runner.rs:31-37, 142-148, 213`
**Severity:** Major (UX + perceived data loss)

When `arboard::Clipboard::new()` fails (headless Linux, ssh without forwarding, sandboxed env), the runner substitutes `MemoryClipboard` whose `write_text` always returns `Ok(())`. The handler at `update.rs:2887-2888` shows the optimistic `Copied: <preview>` toast on every right-click regardless. The user thinks they shared a log line; they actually shared nothing. The runtime failure-toast path (`runner.rs:347-352`) never fires because no `Err` ever returns.

**Fix options:** (a) Replace runtime fallback with a `NullClipboard` whose `write_text` returns `Err`, so the existing failure-toast fires the first time the user right-clicks. (b) Push a startup `ToastLevel::Warn` toast when the fallback is taken, so the user knows before they right-click.

### 3. `docs/ARCHITECTURE.md:1666` — `(?1003 DECSET)` is factually wrong
**Source:** bug_fix_reviewer, architecture_enforcer, code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer (5 of 6 reviewers)
**File:** `docs/ARCHITECTURE.md:1666`
**Severity:** Major (factually inverts the fix; explicit boundary violation per task 10 spec)

The `SetMouseCapture(bool)` description reads "enable or disable terminal mouse capture (`?1003` DECSET)". `?1003` is exactly the mode this entire bug fix removes. The actual modes are `?1000`/`?1002`/`?1006`. Task 10 explicitly forbade naming `?1003` in `ARCHITECTURE.md` ("terminal-protocol detail, not architectural concern"). TASKS.md flagged this at validation time but the fix was not applied.

**Fix:** Delete the parenthetical (one-token edit). Optionally replace with a structural note like "performs the synchronous terminal write outside the TEA pipeline".

### 4. `set_mouse_capture(false)` doc-comment overpromises error surfacing
**Source:** bug_fix_reviewer, architecture_enforcer, code_quality_inspector, risks_tradeoffs_analyzer
**File:** `crates/fdemon-tui/src/terminal.rs:199-201`
**Severity:** Major (active misrepresentation)

The doc-comment says `set_mouse_capture(false)` "calls `disable_mouse_capture` and surfaces any write error as a `Result`, unlike the bare `disable_mouse_capture()` which swallows errors." The implementation at lines 233-234 delegates to `disable_mouse_capture()` (which swallows) and unconditionally returns `Ok(())`. Future maintainers reading the contract will trust an `Err` that will never arrive on the disable path. Also enables risk #1 — runner sees `Ok(())` and trusts terminal state changed even when the write actually failed.

**Fix:** Either update the doc-comment to say "disable-path write errors are logged at `warn` but not returned", or rework `disable_mouse_capture` to return `Result<()>` and propagate.

---

## Minor Findings (Should Fix)

### 5. Vacuous test assertion in `test_status_info_renders_mouse_off_badge`
**Source:** bug_fix_reviewer, code_quality_inspector
**File:** `crates/fdemon-tui/src/widgets/log_view/tests.rs:2523-2526`

Asserts `!buffer_contains("[mouse]") || buffer_contains("[mouse-off]")` — always `true` once `[mouse-off]` is present (substring match makes both sides true). The test cannot detect the regression it claims to detect. Rewrite using exact-match or pattern that excludes `[mouse-off]` as a substring.

### 6. Magic number `60` for toast preview length
**Source:** code_quality_inspector
**File:** `crates/fdemon-app/src/handler/update.rs:2887`

`truncate_with_ellipsis(&entry_text, 60)` — extract `const COPY_TOAST_PREVIEW_CHARS: usize = 60` and reference from both the call site and the test assertion bound (`<=61`).

### 7. EXCEPTION annotation on `mouse_regions` write does not match required style
**Source:** code_quality_inspector
**File:** `crates/fdemon-app/src/handler/mouse/mod.rs:130`

CODE_STANDARDS.md "Region Registry Pattern → Annotation requirement" mandates verbatim text including `(TEA)` qualifier and cross-references to both `CODE_STANDARDS.md` and `REVIEW_FOCUS.md`. Current text is paraphrased and missing the second cross-reference.

### 8. Shift+Alt+m silently does nothing
**Source:** logic_reasoning_checker
**File:** `crates/fdemon-tui/src/event.rs:117`, `crates/fdemon-app/src/handler/keys.rs:23`

`event.rs` canonicalises both `Char('m')|ALT` and `Char('M')|ALT|SHIFT` to `CharAlt(c)`, but `keys.rs` only matches `CharAlt('m')` (lowercase). A user holding Shift gets no toggle and no diagnostic. Either widen the arm to `CharAlt('m' | 'M')` or add an explicit no-op arm with a debug log.

### 9. Alt+m unconditionally swallowed in NewSessionDialog/Startup mode
**Source:** risks_tradeoffs_analyzer
**File:** `crates/fdemon-app/src/handler/keys.rs:23-32`

The `in_text_input` predicate returns `true` for entire `Startup`/`NewSessionDialog` modes, even when the user is in the device-list pane (no text field focused). The toggle is unreachable from fdemon's most-used dialog precisely when it might be needed (e.g., user notices Shift+drag drift while on device picker). Mirror Settings' `editing` check to make the gate field-focus-sensitive.

### 10. Test gaps: Settings-editing & NewSessionDialog Alt+m suppression
**Source:** bug_fix_reviewer
**File:** `crates/fdemon-app/src/handler/tests.rs`

Of the three Alt+m suppression paths (SearchInput, Settings inline edit, NewSessionDialog), only SearchInput is tested. The other two suppression branches have no regression coverage.

### 11. `resolve_entry_text` lacks a focused unit test
**Source:** bug_fix_reviewer, code_quality_inspector, risks_tradeoffs_analyzer
**File:** `crates/fdemon-app/src/handler/update.rs`

Exercised only via two integration-style tests on the full `update()` pipeline. A focused unit test (covering: no session, session with no matching entry, session with matching entry) would localize regressions cleanly.

### 12. `MemoryClipboard` reachable in production (no `#[cfg(test)]` gate)
**Source:** security_reviewer (MEDIUM)
**File:** `crates/fdemon-app/src/services/clipboard.rs:81`, `services/mod.rs:39`

`MemoryClipboard` is `pub` on a non-test path. Future contributors (or the headless runner) could substitute it in production silently. Gate it behind `#[cfg(any(test, feature = "test-helpers"))]`. The runtime fallback in `run()` should use a `NullClipboard` (per finding #2) instead.

---

## Nitpick / Track-as-Debt

### 13. `pending_runner_actions` field `pub` on `AppState`
**Source:** architecture_enforcer, risks_tradeoffs_analyzer
**File:** `crates/fdemon-app/src/state.rs:1218`

The field is never accessed directly by `fdemon-tui`; the legitimate accessor is `Engine::drain_runner_actions()`. The `pub` visibility risks a future second drain site (e.g., the headless runner). The field comment falsely claims `pub` is needed for direct runner access. **Note:** `pub(crate)` would not work cross-crate; instead remove the public visibility and rely on the existing `drain_runner_actions()` accessor only.

### 14. No compile-time enforcement of `UpdateAction` routing
**Source:** risks_tradeoffs_analyzer
**File:** `crates/fdemon-app/src/process.rs:78-82`, `crates/fdemon-tui/src/runner.rs:354-359`

Two `UpdateAction` variants (`SetMouseCapture`, `WriteClipboard`) bypass `handle_action` and are routed via the `pending_runner_actions` queue. Future contributors adding a new variant have no compile-time guidance about routing; mistakes are caught only by `_ => warn!(...)` discard arms. Consider splitting into `AsyncAction` + `RunnerAction` enums, or at minimum add a `debug_assert!(state.pending_runner_actions.is_empty())` invariant at the end of `process_message()`.

### 15. `_ => warn!(...)` catch-all in `handle_runner_actions`
**Source:** code_quality_inspector
**File:** `crates/fdemon-tui/src/runner.rs:354-359`

CODE_STANDARDS.md prefers exhaustive matches. A new runner-only `UpdateAction` variant added in the future would silently log a warning and be discarded. Replace `_` with explicit unhandled variants so the compiler enforces awareness.

### 16. Optimistic toast before write succeeds
**Source:** security_reviewer (LOW), risks_tradeoffs_analyzer
**File:** `crates/fdemon-app/src/handler/update.rs:2887-2888`

The "Copied: …" toast is pushed by the TEA handler before the runner attempts the actual clipboard I/O. On runtime write failure, the user sees both `Copied: Foo` and `Clipboard write failed` — confusing sequencing. Defer the success toast to `handle_runner_actions` after a real `Ok(())` from the clipboard.

### 17. `Alt+m` is the only toggle binding (not configurable)
**Source:** risks_tradeoffs_analyzer
**File:** `crates/fdemon-app/src/handler/keys.rs:23` + the new `docs/MOUSE.md` IDE-terminal matrix

The uncommitted MOUSE.md addition documents that Alt-modified keys are eaten by Zed, VS Code, Cursor, Windsurf, JetBrains-2025-macOS, and Fleet. On those platforms the toggle is unreachable — exactly where Shift+drag drift makes it most needed. The fix philosophy ("toggle off, use native selection") collapses on the IDE terminals where the bug actually shows up. Add `[ui] mouse_toggle_key = "alt+m"` config (default Alt+m, allow `f8`, `ctrl-backslash`, etc.).

### 18. Right-click rewrite couples to `ClickLogRow` variant
**Source:** logic_reasoning_checker, risks_tradeoffs_analyzer (LOW)
**File:** `crates/fdemon-app/src/handler/mouse/mod.rs:139`

Correct today (only the log view emits `ClickLogRow`), but a future refactor that broadens or renames `ClickLogRow` will silently change right-click semantics. Track Option A migration (per-region `on_right` field on `MouseRegionEntry`) as future cleanup.

### 19. `truncate_with_ellipsis` operates on Unicode scalar values, not grapheme clusters
**Source:** logic_reasoning_checker
**File:** `crates/fdemon-app/src/handler/update.rs`

A flag emoji or family-zwj sequence at the 60-char boundary may be split mid-cluster. Acceptable for a status toast (BUG.md scope rejects grapheme-aware selection). Document the contract explicitly.

### 20. PLAN.md cross-reference is prose path, not markdown hyperlink
**Source:** code_quality_inspector, prior validator
**File:** `workflow/plans/features/mouse-support/PLAN.md:536`

`workflow/plans/bugs/log-text-selection-broken/BUG.md` should be `[BUG.md](../bugs/log-text-selection-broken/BUG.md)`. One-line fix.

### 21. Manual-test matrix not executed
**Source:** risks_tradeoffs_analyzer
**File:** `BUG.md:170-181`, `TASKS.md:86`

The two-terminal pre-merge gate (one macOS, one Linux) is unchecked. The acceptance criteria explicitly say "verified by spec, not yet by manual testing". Run the matrix and check the boxes before opening the PR.

### 22. ANSI escape sequences in toast preview
**Source:** security_reviewer (info)
**File:** `crates/fdemon-app/src/handler/update.rs:2887`

Log lines with embedded `\x1b[31m` etc. render as visible characters in the toast (not interpreted as colors). Cosmetic; consider stripping ANSI from `preview` before push.

---

## Documentation Freshness Check

The fix updated `docs/ARCHITECTURE.md`, `docs/MOUSE.md`, `docs/KEYBINDINGS.md`, `docs/CONFIGURATION.md`, and `workflow/plans/features/mouse-support/PLAN.md`. Net assessment:

- ✅ All major doc surfaces updated.
- ❌ `docs/ARCHITECTURE.md:1666` boundary violation (finding #3) and inaccurate `?1003` reference must be fixed.
- ⚠️ `docs/MOUSE.md` IDE-terminal matrix is uncommitted; should be staged with the rest of the merge.
- ⚠️ `terminal.rs:199-201` doc-comment (finding #4) is the implementation-side correlate of the same documentation drift.

No new modules, build steps, or coding patterns were added that lack doc coverage.

---

## Strengths

- **Root-cause analysis (BUG.md) is exemplary.** The `?1003` + dropped Moved-events double-bind is correctly identified and tested.
- **DECSET regression test** (`terminal.rs::test_enable_decset_omits_1003`) pins the omission with an explicit byte-window check that cannot produce false positives.
- **Clean TEA decomposition** across 10 tasks with explicit wave/dependency declarations let work fan out safely. Validators caught most issues at task time even if not all were fixed before merge.
- **Service abstraction** (`Clipboard` trait + `MemoryClipboard` mock + `SystemClipboard` arboard impl) cleanly accommodates headless CI.
- **No layer-boundary violations** — `arboard` is correctly placed in `fdemon-app` (the orchestration layer), the runner depends on `fdemon-app` as expected, no inversion anywhere.
- **No security findings above MEDIUM.** The right-click flow has implicit user consent; no escape-sequence injection surface; no credentials handling.
- **IDE-terminal compatibility matrix** (uncommitted MOUSE.md addition) is unusually thorough — eight IDEs analyzed with upstream issue links and per-IDE workarounds.

---

## Recommendation

**Block merge until findings #1–#4 are addressed** (two correctness defects + two documentation regressions). #5–#12 should be fixed in the same PR or filed as immediate follow-ups. #13–#22 can be tracked as known debt.

See [ACTION_ITEMS.md](./ACTION_ITEMS.md) for the prioritized punch list.
