# Action Items: log-text-selection-broken

**Review Date:** 2026-05-15
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 4 (2 correctness, 2 docs)
**Should-fix:** 8
**Nice-to-have / debt:** 10

Cross-reference: [REVIEW.md](./REVIEW.md)

---

## Critical (Must Fix Before Merge)

### 1. Fix `MouseCaptureChanged` `try_send` correctness bug

- **Source:** bug_fix_reviewer, security_reviewer, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-tui/src/runner.rs:331-335`
- **Problem:** On a full message channel, `try_send(MouseCaptureChanged { active: target })` drops the message. `AppState::mouse_capture_active` then never updates, the `[mouse]`/`[mouse-off]` badge lies indefinitely, and the next `Alt+m` press computes the wrong target (often a no-op via the idempotency guard) — leaving the user with no visible recovery path.
- **Required action:** Either (a) switch to `blocking_send().await` (the runner is already in a Tokio context), or (b) on `try_send` failure, write `state.mouse_capture_active = target` directly and push an error toast — the runner already mutates `state.toasts` at line 343-344.
- **Acceptance:** Add a unit test that simulates a full channel and verifies `state.mouse_capture_active` reflects the target value either way.

### 2. Replace silent `MemoryClipboard` runtime fallback with a `NullClipboard`

- **Source:** security_reviewer (MEDIUM), risks_tradeoffs_analyzer (HIGH)
- **File:** `crates/fdemon-tui/src/runner.rs:31-37, 142-148, 213` and `crates/fdemon-app/src/services/clipboard.rs`
- **Problem:** When `arboard::Clipboard::new()` fails, the runner substitutes `MemoryClipboard` whose `write_text` always returns `Ok(())`. The user gets the optimistic `Copied: <preview>` toast for every right-click while the OS clipboard remains untouched. They paste elsewhere and find stale content — silent data loss.
- **Required action:** Add a `NullClipboard` whose `write_text` returns `Err(Error::terminal("system clipboard unavailable"))`. Substitute it (not `MemoryClipboard`) in all three `runner.rs` fallback sites. The existing failure-toast path (`runner.rs:347-352`) will then fire on the user's first right-click, giving them a visible signal.
- **Acceptance:** New unit test in `runner.rs` verifying that `WriteClipboard` action against `NullClipboard` produces a `ToastLevel::Warn` toast containing "Clipboard write failed" and does NOT silently succeed.

### 3. Delete `(?1003 DECSET)` from `docs/ARCHITECTURE.md:1666`

- **Source:** bug_fix_reviewer, architecture_enforcer, code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer (5 of 6 reviewers)
- **File:** `docs/ARCHITECTURE.md:1666`
- **Problem:** The `SetMouseCapture(bool)` description says "enable or disable terminal mouse capture (`?1003` DECSET)". `?1003` is exactly the mode this whole bug fix removes. Task 10 explicitly forbade naming `?1003` in this doc ("terminal-protocol detail, not architectural concern"). TASKS.md flagged this at validation but the fix was never applied.
- **Required action:** Change line 1666 to "Instruct the TUI runner to enable or disable terminal mouse capture. The runner performs the synchronous terminal write outside the TEA pipeline." — or just delete the parenthetical.
- **Acceptance:** `grep -n '1003' docs/ARCHITECTURE.md` returns no matches.

### 4. Correct the `set_mouse_capture` doc-comment on the disable path

- **Source:** bug_fix_reviewer, architecture_enforcer, code_quality_inspector, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-tui/src/terminal.rs:199-201`
- **Problem:** Doc says `set_mouse_capture(false)` "surfaces any write error as a `Result`, unlike the bare `disable_mouse_capture()` which swallows errors." Implementation at lines 233-234 unconditionally returns `Ok(())` even when the underlying write fails. This contributes to bug #1 — the runner trusts `Ok(())` and tells the model the terminal flipped state when it didn't.
- **Required action:** Either (a) update the doc-comment to accurately say "disable-path write errors are logged at `warn` level but are not returned; only the enable path surfaces write errors", or (b) make `disable_mouse_capture()` return `Result<()>` and propagate.
- **Acceptance:** A reader of the doc-comment cannot misinfer the disable-path behavior.

---

## Major (Should Fix in Same PR)

### 5. Rewrite vacuous test assertion in `test_status_info_renders_mouse_off_badge`

- **Source:** bug_fix_reviewer, code_quality_inspector
- **File:** `crates/fdemon-tui/src/widgets/log_view/tests.rs:2523-2526`
- **Problem:** `!buffer_contains("[mouse]") || buffer_contains("[mouse-off]")` is a tautology once `[mouse-off]` is present (substring match makes both halves of the disjunction true). The test cannot detect what it claims to detect.
- **Suggested action:** Use an exact-match helper, or pattern that explicitly excludes `[mouse-off]` substring (e.g., scan the buffer for `[mouse]` not followed by `-`).

### 6. Extract `60` magic number to a named constant

- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/handler/update.rs:2887`
- **Problem:** `truncate_with_ellipsis(&entry_text, 60)` violates CODE_STANDARDS.md "Magic numbers". The user-facing limit is also documented in `docs/MOUSE.md` ("60-char preview"), so it must stay in sync across three places.
- **Suggested action:** Add `const COPY_TOAST_PREVIEW_CHARS: usize = 60;` adjacent to the handler arm; reference from both the call site and the test bound (`<= 61` in `test_copy_message_truncates_preview_to_60_chars`).

### 7. Conform the `mouse_regions` EXCEPTION annotation to required style

- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/handler/mouse/mod.rs:130`
- **Problem:** CODE_STANDARDS.md "Region Registry Pattern → Annotation requirement" mandates verbatim text including `(TEA)` qualifier and cross-references to both `CODE_STANDARDS.md` and `REVIEW_FOCUS.md`. Current text is paraphrased and missing the second cross-reference.
- **Suggested action:** Match the standard's exact wording.

### 8. Decide Shift+Alt+m policy

- **Source:** logic_reasoning_checker
- **File:** `crates/fdemon-app/src/handler/keys.rs:23`
- **Problem:** `event.rs` canonicalises both `Char('m')|ALT` and `Char('M')|ALT|SHIFT` to `CharAlt(c)`, but `keys.rs` matches only `CharAlt('m')`. Shift-holding users get no toggle and no diagnostic.
- **Suggested action:** Widen to `CharAlt('m' | 'M')` (recommended), or add an explicit no-op arm for `CharAlt('M')` with a debug log so future debugging is easier.

### 9. Make Alt+m suppression in NewSessionDialog/Startup field-focus-sensitive

- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/keys.rs:23-32`
- **Problem:** The current `in_text_input` predicate returns `true` for the entire `Startup`/`NewSessionDialog` modes, even when the user is in the device-list pane (no text field focused). The toggle is unreachable from fdemon's most-used dialog precisely when the user might be discovering Shift+drag drift.
- **Suggested action:** Mirror Settings' `editing` check — suppress only when a text field is actively focused.

### 10. Add Alt+m suppression tests for Settings-editing and NewSessionDialog modes

- **Source:** bug_fix_reviewer
- **File:** `crates/fdemon-app/src/handler/tests.rs`
- **Problem:** Only `SearchInput` suppression is tested. The other two suppression paths have no regression coverage.
- **Suggested action:** Add `test_alt_m_in_settings_editing_does_not_toggle` and `test_alt_m_in_new_session_dialog_text_field_does_not_toggle` (the latter contingent on action item #9).

### 11. Add focused unit test for `resolve_entry_text`

- **Source:** bug_fix_reviewer, code_quality_inspector, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/update.rs`
- **Problem:** Exercised only via two integration-style tests on the full `update()` pipeline. A regression in `resolve_entry_text` surfaces as "WriteClipboard text doesn't contain expected substring" rather than the actual root cause.
- **Suggested action:** Add direct unit test covering: no active session, session with no matching entry, session with matching entry.

### 12. Gate `MemoryClipboard` behind a test feature flag

- **Source:** security_reviewer (MEDIUM)
- **File:** `crates/fdemon-app/src/services/clipboard.rs:81`, `crates/fdemon-app/src/services/mod.rs:39`
- **Problem:** `MemoryClipboard` is `pub` on a non-test path. A future contributor or downstream crate could substitute it in production silently. Pairs with action #2 — runtime fallback should use `NullClipboard`, not `MemoryClipboard`.
- **Suggested action:** Mark with `#[cfg(any(test, feature = "test-helpers"))]` and adjust the re-export in `services/mod.rs`.

---

## Minor / Track as Debt

### 13. Narrow `pending_runner_actions` visibility

- **Source:** architecture_enforcer, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/state.rs:1218`
- The field is `pub` on `AppState`; the only legitimate accessor is `Engine::drain_runner_actions()`. `pub(crate)` would not work cross-crate, but the field comment falsely claims it is `pub` for direct runner access. Remove the field-level `pub` or add a clear "do not drain directly" doc-comment.

### 14. Add compile-time enforcement of `UpdateAction` routing

- **Source:** risks_tradeoffs_analyzer
- **Files:** `crates/fdemon-app/src/process.rs:78-82`, `crates/fdemon-tui/src/runner.rs:354-359`
- Two `UpdateAction` variants bypass `handle_action`. Mistakes are only caught by `_ => warn!(...)` discard arms. Track: split into `AsyncAction` + `RunnerAction` enums, or add `debug_assert!(state.pending_runner_actions.is_empty())` invariant at the end of `process_message()`.

### 15. Replace `_ => warn!(...)` catch-all in `handle_runner_actions`

- **Source:** code_quality_inspector
- **File:** `crates/fdemon-tui/src/runner.rs:354-359`
- Exhaustively list both variants so the compiler enforces awareness when a third runner-only action lands.

### 16. Defer success toast to the runner

- **Source:** security_reviewer (LOW), risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/update.rs:2887-2888`
- The "Copied: …" toast fires from the TEA handler before the runner attempts the actual clipboard I/O. On runtime failure, the user sees both `Copied: Foo` and `Clipboard write failed` — confusing sequencing. Move the success toast to `handle_runner_actions` after a real `Ok(())`. Pairs with action #2.

### 17. Add `[ui] mouse_toggle_key` config option

- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/keys.rs:23` + `Settings`
- The newly-documented MOUSE.md "IDE built-in terminals" matrix shows that Alt-modified keys are eaten by Zed, VS Code, Cursor, Windsurf, JetBrains-2025-macOS, and Fleet — exactly the platforms where Shift+drag drift makes the toggle most needed. Make the binding configurable (default `alt+m`, accept `f8`, `ctrl-backslash`, etc.).

### 18. Track Option A migration for right-click handling

- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer (LOW)
- **File:** `crates/fdemon-app/src/handler/mouse/mod.rs:139`
- The current rewrite-and-dedup pattern matches only `Message::ClickLogRow` and is correct today, but couples right-click semantics to that one variant. Track migration to per-region `on_right` field on `MouseRegionEntry` as future cleanup.

### 19. Document `truncate_with_ellipsis` Unicode contract

- **Source:** logic_reasoning_checker
- **File:** `crates/fdemon-app/src/handler/update.rs`
- The function operates on Unicode scalar values, not grapheme clusters — a flag emoji or family-zwj sequence at the boundary may be split mid-cluster. Document the contract; no behavior change required.

### 20. Convert PLAN.md prose path to markdown hyperlink

- **Source:** code_quality_inspector, prior validator
- **File:** `workflow/plans/features/mouse-support/PLAN.md:536`
- Change `workflow/plans/bugs/log-text-selection-broken/BUG.md` → `[BUG.md](../bugs/log-text-selection-broken/BUG.md)`.

### 21. Execute the manual-test matrix

- **Source:** risks_tradeoffs_analyzer
- **File:** `BUG.md:170-181`, `TASKS.md:86`
- The two-terminal pre-merge gate (one macOS, one Linux) is unchecked. Run the matrix and check the boxes before opening the PR. Note: the user has already reported that the fix breaks in Zed — that's not a regression but it does mean the matrix should explicitly cover IDE terminals too.

### 22. Strip ANSI escape sequences from toast preview

- **Source:** security_reviewer (info)
- **File:** `crates/fdemon-app/src/handler/update.rs:2887`
- Optional cosmetic improvement: log lines with embedded `\x1b[31m` etc. render as visible characters in the toast.

---

## Re-review Checklist

After addressing critical and major issues, verify:

- [ ] `MouseCaptureChanged` is delivered (or state synced directly) under channel pressure (#1)
- [ ] Right-click on a host without OS clipboard surfaces a visible warning toast (#2)
- [ ] `grep -n '1003' docs/ARCHITECTURE.md` returns no matches (#3)
- [ ] `set_mouse_capture` doc-comment matches its actual behavior on both branches (#4)
- [ ] `test_status_info_renders_mouse_off_badge` actually fails when `[mouse]` (without `-off`) is present (#5)
- [ ] No `60` magic number in `update.rs`; replaced by `COPY_TOAST_PREVIEW_CHARS` (#6)
- [ ] EXCEPTION annotation matches required verbatim style (#7)
- [ ] Shift+Alt+m policy is decided and either fires the toggle or logs a diagnostic (#8)
- [ ] Alt+m suppression in NewSessionDialog text fields verified by test (#9, #10)
- [ ] `resolve_entry_text` has at least one focused unit test (#11)
- [ ] `MemoryClipboard` is not constructable from production paths (#12)
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes (~5571+ tests)
- [ ] Manual-test matrix executed on at least one stand-alone macOS terminal and one Linux terminal (#21)

After all critical and major items are addressed, dispatch a re-review of the modified files only (no need to re-review files that didn't change).
