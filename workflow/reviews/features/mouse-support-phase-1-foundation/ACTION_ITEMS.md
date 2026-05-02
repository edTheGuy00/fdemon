# Action Items: Mouse Support — Phase 1 Foundation

**Review Date:** 2026-05-02
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 1 critical + 4 major

## Critical Issues (Must Fix)

### 1. Clippy `assertions_on_constants` failure blocks Phase 1 success criteria
- **Source:** code_quality_inspector, architecture_enforcer, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/input_mouse.rs`
- **Lines:** 182, 183, 184
- **Problem:** `assert!(!KeyModSet::NONE.shift)` (and `.ctrl`, `.alt`) — `KeyModSet::NONE` is a `const`, so each assertion reduces to `assert!(false)` at compile time, which `clippy::assertions_on_constants` rejects under `-D warnings`. Already documented in TASKS.md line 44 as a pre-merge concern; not yet fixed.
- **Required Action:** Replace with a local `let` binding before asserting:
  ```rust
  fn test_keymodset_none_is_empty() {
      let none = KeyModSet::NONE;
      assert!(!none.shift);
      assert!(!none.ctrl);
      assert!(!none.alt);
  }
  ```
  Do NOT use `#[allow(clippy::assertions_on_constants)]` — the lint protects against future regressions on actually-runtime values.
- **Acceptance:** `cargo clippy -p fdemon-app --all-targets -- -D warnings` passes with zero warnings.

## Major Issues (Should Fix)

### 1. Add missing integration test for `update(state, Message::Mouse(...))`
- **Source:** code_quality_inspector, logic_reasoning_checker
- **File:** `crates/fdemon-app/src/handler/tests.rs` (new test)
- **Problem:** Plan acceptance criterion at TASKS.md line 87 explicitly requires it; only `handle_mouse` is unit-tested today, not the `update()` routing in `handler/update.rs:60-66`. A regression that wires `Message::Mouse` to a side effect would silently pass CI.
- **Suggested Action:**
  ```rust
  #[test]
  fn test_mouse_message_returns_none_result_and_does_not_mutate_state() {
      let mut state = AppState::new();
      let before = state.ui_mode;
      let result = update(&mut state, Message::Mouse(MouseInput::Click {
          x: 0, y: 0, button: MouseButton::Left, modifiers: KeyModSet::NONE,
      }));
      assert!(result.message.is_none());
      assert!(result.action.is_none());
      assert_eq!(state.ui_mode, before);
  }
  ```

### 2. Add `///` doc comments to public functions in `event.rs`
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-tui/src/event.rs`
- **Problem:** `pub fn key_event_to_input` and `pub fn poll` are public API but lack `///` headers, violating `docs/CODE_STANDARDS.md` documentation requirements.
- **Suggested Action:** Add purpose, return semantics (especially `None`-returning paths and `Moved`-drop behavior), and any caller obligations.

### 3. Correct the "no behavior change" claim in TASKS.md
- **Source:** risks_tradeoffs_analyzer, security_reviewer
- **File:** `workflow/plans/features/mouse-support/phase-1-foundation/TASKS.md` (line 7)
- **Problem:** Statement "a user can scroll/click anywhere in fdemon and nothing changes" is false — enabling DECSET 1000/1002/1003/1015/1006 silently breaks native text-selection in many terminals and intercepts wheel-scroll that previously moved host scrollback.
- **Required Action (one of):**
  - (a) Reword to "no fdemon TEA-state change; user-visible terminal behavior intentionally changes (text selection, native scrollback)"; document the trade-off in `docs/CONFIGURATION.md`.
  - (b) Flip default to `enable_mouse: false` for Phase 1; revisit in Phase 2 once a visible benefit lands. (Strongly preferred by risks_tradeoffs_analyzer.)
- **Acceptance:** Either the claim is accurate, or the default is off.

### 4. Run cross-platform manual smoke test
- **Source:** risks_tradeoffs_analyzer
- **Problem:** Phase 1 acceptance criteria only cover macOS. Windows risk profile is unverified despite known crossterm bugs (#613, #986) and the legacy-conhost silent no-op case.
- **Required Action:**
  - Reproduce TASKS.md success-criteria smoke tests on Windows (Windows Terminal — and legacy conhost if accessible) and Linux (gnome-terminal + tmux without `mouse on`).
  - Confirm: panic recovery, normal exit, `enable_mouse=false`, terminal usability after Ctrl+C.
  - Document any platform degradation in `docs/CONFIGURATION.md` (new section: "Mouse Capture — Terminal Compatibility").
- **Acceptance:** Smoke tests recorded as passed on at least Linux + Windows in addition to macOS.

### 5. Add `[ui] enable_mouse` to user-facing config docs
- **Source:** doc_freshness
- **File:** `docs/CONFIGURATION.md`
- **Problem:** The `[ui]` settings table (lines 308-326) does not include the new `enable_mouse` field. User-facing settings must be documented.
- **Suggested Action:** Dispatch `doc_maintainer` to add the row to the table and the example block. Include the "restart required" note and any platform caveats discovered in M4.

## Minor Issues (Consider Fixing)

### 1. Downgrade `Ordering::SeqCst` → `Release`/`Acquire` on `MOUSE_CAPTURE_ON`
- **Source:** code_quality_inspector, logic_reasoning_checker, security_reviewer
- **File:** `crates/fdemon-tui/src/terminal.rs` (lines 58, 72, 91, 103, 114, 118, 128)
- **Action:** `Release` on `enable`'s store; `Acquire` (or `AcqRel`) on `disable`'s swap; `Relaxed` on test-only resets. Documents the actual happens-before requirement and avoids unnecessary cross-thread total ordering.

### 2. Make `install_panic_hook()` idempotent
- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer
- **File:** `crates/fdemon-tui/src/terminal.rs`
- **Action:** Add `static HOOK_INSTALLED: AtomicBool` guard; early-return if already installed. Mirrors the `MOUSE_CAPTURE_ON` pattern in the same file.

### 3. Rename `MouseInput::Click` → `Press` (or document loudly)
- **Source:** logic_reasoning_checker
- **File:** `crates/fdemon-app/src/input_mouse.rs`
- **Action:** Variant is emitted on `MouseEventKind::Down` (press semantics), not on a debounced click. Cheap to fix now (no public consumers exist yet); breaking change once Phase 2 ships.

### 4. Add discoverability for `enable_mouse: false` off-switch
- **Source:** risks_tradeoffs_analyzer
- **Action:** Either flip default (see Major #3 alternative b), or add a one-time first-launch hint ("Mouse on — Shift+drag for native selection; toggle in Settings → UI") gated on `ui.mouse_hint_seen`.

## Nitpicks (Optional)

1. Expand `crates/fdemon-tui/src/event.rs` module `//!` header to mention key + mouse + polling responsibilities.
2. Add `cmd: bool` to `KeyModSet`; add `lines: u8` (or similar delta) to `MouseInput::Scroll` — easier additive change now than after Phase 2.
3. Document the DECSET 1003 trade-off near `EnableMouseCapture` call (any-motion mode is enabled even though `Moved` is dropped at the boundary).
4. Add a one-line comment in `terminal.rs::install_panic_hook` explaining why `disable_mouse_capture` before `ratatui::restore` is safe (DECRST modes are connection-global, not alt-screen-scoped).
5. Replace `assert_eq!(items.len(), 35)` with a per-id existence check in `widgets/settings_panel/tests.rs` to stop the per-setting churn.
6. Configure `insta` to filter the crate version line out of snapshot headers (`[tool.insta] redactions` or global `with_settings!{ filters }`) to stop per-release snapshot tax.

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] All critical issues resolved (clippy gate green)
- [ ] All major issues resolved or explicitly justified (with TODO + tracking link)
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] Manual smoke tests recorded on macOS + Linux + Windows
