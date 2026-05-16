# Bugfix Plan: log-text-selection-broken — review follow-up

**Parent fix:** [`workflow/plans/bugs/log-text-selection-broken/`](../log-text-selection-broken/)
**Review:** [`workflow/reviews/bugs/log-text-selection-broken/REVIEW.md`](../../../reviews/bugs/log-text-selection-broken/REVIEW.md)
**Action items:** [`workflow/reviews/bugs/log-text-selection-broken/ACTION_ITEMS.md`](../../../reviews/bugs/log-text-selection-broken/ACTION_ITEMS.md)

## TL;DR

The 6-agent code review of the log-text-selection / copy fix returned **NEEDS WORK** with 4 blocking, 8 should-fix, and 10 debt items. This follow-up plan addresses all 4 blockers, all 8 should-fix items, and 6 of the 10 debt items in a single PR. The remaining 4 debt items (`UpdateAction` enum split, configurable `mouse_toggle_key`, right-click Option A migration, ANSI-strip in toast preview) are deferred — they each warrant their own scope and are listed under [Future Enhancements](#future-enhancements).

---

## Bug Reports

### Bug 1: `MouseCaptureChanged` follow-up dropped on full channel — state drifts permanently
**Symptom:** After `Alt+m`, the `[mouse]`/`[mouse-off]` badge can permanently disagree with the terminal's actual capture mode. Subsequent `Alt+m` presses compute the wrong target and become no-ops via the idempotency guard, leaving the user with no recovery short of restart.

**Expected:** The TEA `mouse_capture_active` field reflects the actual terminal state, per the BUG.md design contract.

**Root Cause Analysis:**
1. `crates/fdemon-tui/src/runner.rs:331-335` uses `try_send` for the follow-up `Message::MouseCaptureChanged { active: target }`.
2. On a full 256-slot mpsc channel, the message is dropped with a `warn!` log only.
3. `AppState::mouse_capture_active` then never updates.

**Affected Files:**
- `crates/fdemon-tui/src/runner.rs` — `handle_runner_actions`

---

### Bug 2: Silent `MemoryClipboard` runtime fallback hides clipboard unavailability
**Symptom:** On a host without a working OS clipboard (headless Linux, ssh without X-forwarding, sandboxed env), right-clicking a log line shows the optimistic `Copied: <preview>` toast while the OS clipboard remains untouched. The user pastes elsewhere and gets stale content — perceived data loss.

**Expected:** A visible warning when the clipboard is unavailable, ideally on the user's first attempted copy (so they know not to rely on it).

**Root Cause Analysis:**
1. `crates/fdemon-tui/src/runner.rs:31-37, 142-148, 213` substitutes `MemoryClipboard` when `SystemClipboard::new()` fails.
2. `MemoryClipboard::write_text` always returns `Ok(())`.
3. The handler at `crates/fdemon-app/src/handler/update.rs:2887-2888` pushes `Copied: <preview>` toast unconditionally before the runner attempts I/O.
4. The existing failure-toast path at `runner.rs:347-352` only fires on `Err(_)` — which never occurs against `MemoryClipboard`.

**Affected Files:**
- `crates/fdemon-app/src/services/clipboard.rs` — add `NullClipboard`
- `crates/fdemon-app/src/services/mod.rs` — re-export wiring
- `crates/fdemon-tui/src/runner.rs` — substitute `NullClipboard` instead of `MemoryClipboard` in fallback paths

---

### Bug 3: `docs/ARCHITECTURE.md` description of `SetMouseCapture` is factually wrong
**Symptom:** Line 1666 reads "enable or disable terminal mouse capture (`?1003` DECSET)". `?1003` is the exact mode this whole bug fix removes. The actual sequence is `?1000h ?1002h ?1006h`.

**Expected:** No terminal-protocol detail in `ARCHITECTURE.md` per task 10's explicit boundary rule, AND no factual misrepresentation either way.

**Root Cause Analysis:**
1. Task 10 (doc_maintainer) ignored its own task spec which explicitly forbade naming `?1003` in this doc.
2. Per-task validator flagged the issue as CONCERN with "one-phrase deletion fixes it" — orchestrator did not block on CONCERN.

**Affected Files:**
- `docs/ARCHITECTURE.md` (line 1666)

---

### Bug 4: `set_mouse_capture` doc-comment overpromises error surfacing on disable path
**Symptom:** `crates/fdemon-tui/src/terminal.rs:199-201` documents `set_mouse_capture(false)` as "surfacing any write error as a `Result`" — but the implementation at lines 233-234 unconditionally returns `Ok(())`, swallowing any DISABLE_MOUSE_DECSET write error inside `disable_mouse_capture()`. Future maintainers will trust an `Err` that will never arrive.

**Expected:** Doc-comment matches implementation behavior.

**Root Cause Analysis:**
1. Task 01 validator flagged this as CONCERN — orchestrator did not block on CONCERN.
2. The disable path delegates to `disable_mouse_capture()` which logs at `warn!` and returns `()`; the wrapper cannot propagate what it cannot observe.

**Affected Files:**
- `crates/fdemon-tui/src/terminal.rs` — doc-comment correction

---

## Affected Modules

- `crates/fdemon-tui/src/runner.rs`: `try_send` correctness fix; substitute `NullClipboard` in fallback paths; replace `_ => warn!` catch-all with explicit unhandled arms.
- `crates/fdemon-app/src/services/clipboard.rs`: add `NullClipboard`; gate `MemoryClipboard` behind `#[cfg(test)]`.
- `crates/fdemon-app/src/services/mod.rs`: re-export wiring.
- `crates/fdemon-tui/src/terminal.rs`: doc-comment correction on `set_mouse_capture(false)` path.
- `crates/fdemon-app/src/handler/update.rs`: extract `COPY_TOAST_PREVIEW_CHARS = 60` constant; document `truncate_with_ellipsis` Unicode-scalar contract.
- `crates/fdemon-app/src/handler/keys.rs`: widen Alt+m match to `'m' | 'M'`; refine NewSessionDialog/Startup suppression to be field-focus-sensitive.
- `crates/fdemon-app/src/handler/mouse/mod.rs`: conform EXCEPTION annotation to required style.
- `crates/fdemon-app/src/handler/tests.rs`: missing Alt+m suppression tests; focused unit test for `resolve_entry_text`.
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`: rewrite vacuous assertion in `test_status_info_renders_mouse_off_badge`.
- `crates/fdemon-app/src/state.rs`: narrow `pending_runner_actions` visibility, fix misleading field comment.
- `docs/ARCHITECTURE.md`: delete `?1003 DECSET` parenthetical (doc_maintainer).
- `workflow/plans/features/mouse-support/PLAN.md`: convert prose path to markdown hyperlink.

---

## Phases

### Phase 1: Service additions and standalone fixes (parallel-safe)

Tasks that touch independent files and can run in parallel worktrees.

**Tasks:**
- `01-null-clipboard-service` — Add `NullClipboard`, cfg-gate `MemoryClipboard`. Prerequisite for the runner-side fix in phase 2.
- `02-terminal-doc-correction` — Fix `set_mouse_capture` disable-path doc-comment.
- `03-architecture-doc-fix` — Remove `?1003 DECSET` from `docs/ARCHITECTURE.md` (doc_maintainer).
- `04-test-and-quality-polish` — Vacuous assertion repair, EXCEPTION annotation, extract `COPY_TOAST_PREVIEW_CHARS` constant, focused `resolve_entry_text` test, Unicode-contract doc.
- `05-keys-and-suppression` — Widen Alt+m to `'m' | 'M'`, refine NewSessionDialog suppression to field-focus, add the missing suppression tests.
- `06-plan-md-hyperlink` — Convert PLAN.md cross-reference to a clickable hyperlink.
- `07-state-visibility` — Narrow `pending_runner_actions` visibility, fix misleading field comment.

### Phase 2: Runner correctness (depends on Phase 1's `NullClipboard`)

**Tasks:**
- `08-runner-correctness` — `try_send` fallback writes `state.mouse_capture_active = target` directly; substitute `NullClipboard` in three fallback sites; replace `_` catch-all with explicit unhandled arms.

### Phase 3: Manual verification gate

**Tasks:**
- `09-manual-test-matrix` — Execute the BUG.md two-terminal verification (one macOS, one Linux). Update parent BUG.md success-criteria checkboxes. Confirm reviewer-flagged Zed/IDE-terminal limitations match the new MOUSE.md "IDE built-in terminals" matrix.

---

## Edge Cases & Risks

### Risk: `try_send` failure direct-write path violates "single update site" TEA invariant
- **Risk:** Mutating `state.mouse_capture_active` from inside `handle_runner_actions` (instead of through a `Message`) bypasses the TEA model's "all state changes go through `update()`" rule.
- **Mitigation:** This is a fallback path triggered only when the message channel is saturated. The mutation reflects an already-observed terminal state change — it's the message that would have been processed if the channel had capacity. Alternative is to swap to `blocking_send().await`, but `run_loop` is synchronous and the codebase universally uses `try_send` for runner-side message emissions; introducing one `blocking_send` site would create an inconsistent pattern. Document the fallback explicitly in the runner.

### Risk: `NullClipboard` adoption changes the no-clipboard UX
- **Risk:** Users who previously experienced "silent success" copies on headless hosts will now see a `Clipboard write failed` toast on every right-click attempt. This is the *desired* behavior, but it's a behavior change.
- **Mitigation:** A startup `ToastLevel::Warn` toast (one-shot when `SystemClipboard::new()` fails) will tell users up-front: "Clipboard unavailable — right-click copy is disabled." Adopting that startup-toast pattern is part of task 08.

### Risk: NewSessionDialog field-focus refinement might re-introduce text input bugs
- **Risk:** Loosening the broad "Startup | NewSessionDialog → suppress all global keys" rule could let `Alt+m` interfere when a user is composing a value in a launch-context text field.
- **Mitigation:** The refinement uses the existing `DialogPane::LaunchContext` discriminator (already used to route key events to the right pane). When `active_pane == TargetSelector` (device picker), no text field is focused — Alt+m is safe to dispatch. When `active_pane == LaunchContext`, suppress as today. Tests cover both branches.

### Risk: doc-comment correction in terminal.rs reveals a deeper design gap
- **Risk:** If reviewers later argue the `set_mouse_capture(false)` path *should* propagate write errors, that's a `disable_mouse_capture()` signature change (return `Result<()>` instead of `()`).
- **Mitigation:** Out of scope for this follow-up — comment-only fix is the minimal correct change. If propagating disable errors becomes important, file a separate issue.

---

## Further Considerations

### `UpdateAction` enum split (deferred)

The reviewers (architecture, risks_tradeoffs) flagged the bifurcated routing between `handle_action` (Tokio side effects) and `pending_runner_actions` (runner side effects) as a maintenance footgun. Splitting into `AsyncAction` + `RunnerAction` enums would give compile-time enforcement.

**Why deferred:** Touches the exhaustive match at `crates/fdemon-app/src/actions/mod.rs:56` (≈25 variants) and ripples through every callsite. Worth its own design phase. Track as separate issue.

### Configurable `mouse_toggle_key` (deferred to a small feature)

The new `docs/MOUSE.md` "IDE built-in terminals" section documents that Alt+m is unreachable on Zed, VS Code, Cursor, Windsurf, JetBrains-2025-macOS, and Fleet — exactly the platforms where Shift+drag drift makes the toggle most needed.

**Why deferred:** Requires (a) `[ui] mouse_toggle_key` field on `UiSettings`, (b) `InputKey::from_str` parser (does not exist anywhere in the codebase yet), (c) handler threading. The parser is reusable infrastructure that other settings could benefit from. Plan as a small feature: `workflow/plans/features/configurable-keybindings/` (TBD).

### Right-click Option A migration (deferred)

The current right-click handler couples to `Message::ClickLogRow` discriminator. Option A (per-region `on_right` field on `MouseRegionEntry`) decouples but requires touching the registry contract.

**Why deferred:** The coupling is benign today; a refactor would be premature without a second copyable surface to motivate it.

### ANSI escape strip in toast preview (deferred)

Cosmetic only. If users frequently see garbled toast previews from ANSI-laden log lines, file as a separate issue with concrete examples.

---

## Task Dependency Graph

```
Phase 1 (parallel-safe)
├── 01-null-clipboard-service
├── 02-terminal-doc-correction
├── 03-architecture-doc-fix              [doc_maintainer]
├── 04-test-and-quality-polish
├── 05-keys-and-suppression
├── 06-plan-md-hyperlink
└── 07-state-visibility

Phase 2 (depends on 01)
└── 08-runner-correctness
    └── depends on: 01

Phase 3 (depends on all of Phases 1+2)
└── 09-manual-test-matrix
    └── depends on: 01-08
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `NullClipboard` exists, `MemoryClipboard` is `#[cfg(test)]`-only.
- [ ] `terminal.rs::set_mouse_capture` doc-comment matches its actual disable-path behavior.
- [ ] `grep -n '1003' docs/ARCHITECTURE.md` returns no matches.
- [ ] `widgets/log_view/tests.rs::test_status_info_renders_mouse_off_badge` actually fails when `[mouse]` (without `-off`) is present.
- [ ] `EXCEPTION` annotation in `handler/mouse/mod.rs:130` matches the verbatim style required by `CODE_STANDARDS.md`.
- [ ] `COPY_TOAST_PREVIEW_CHARS` constant replaces literal `60` in `handler/update.rs` and the test bound.
- [ ] `truncate_with_ellipsis` doc-comment notes the Unicode-scalar contract.
- [ ] `resolve_entry_text` has at least one focused unit test covering the no-session, missing-entry, and matching-entry paths.
- [ ] `Alt+m` policy decided for Shift+Alt+m (widened to `'m' | 'M'` recommended).
- [ ] `NewSessionDialog`/`Startup` suppression is field-focus-sensitive — `Alt+m` works when the device picker pane is focused.
- [ ] `Settings`-editing and `NewSessionDialog`-text-field Alt+m suppression have regression tests.
- [ ] `PLAN.md` cross-reference is a clickable markdown hyperlink.
- [ ] `pending_runner_actions` field is no longer `pub`, or has an updated doc-comment if cross-crate `pub` is unavoidable.

### Phase 2 Complete When:
- [ ] `MouseCaptureChanged` `try_send` failure path mutates `state.mouse_capture_active` directly (or a unit test proves the state stays consistent under simulated channel pressure).
- [ ] Three runner-fallback sites use `NullClipboard` (not `MemoryClipboard`).
- [ ] Right-click against `NullClipboard` produces a `ToastLevel::Warn` toast containing "Clipboard write failed".
- [ ] Startup `ToastLevel::Warn` toast fires when `SystemClipboard::new()` fails, telling the user clipboard is unavailable.
- [ ] `handle_runner_actions` exhaustively matches both runner-side variants (no `_` catch-all).

### Phase 3 Complete When:
- [ ] BUG.md manual-test matrix is checked off for at least one stand-alone macOS terminal and one stand-alone Linux terminal.
- [ ] Reviewer-flagged Zed/IDE-terminal limitations are confirmed to match the new MOUSE.md "IDE built-in terminals" matrix (no further code changes implied).

---

## Milestone Deliverable

After all 9 follow-up tasks complete, the parent fix's review verdict moves from **NEEDS WORK** to **APPROVED**. The PR is ready to merge with documented limitations (IDE-terminal matrix in MOUSE.md) and explicit deferred work (`UpdateAction` enum split, configurable toggle key, right-click Option A migration) tracked as separate efforts.
