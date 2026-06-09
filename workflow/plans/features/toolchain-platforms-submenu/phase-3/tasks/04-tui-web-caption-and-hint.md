## Task: TUI — Web leaf caption + suppress "coming soon" when guided commands exist

**Objective**: Render the live `PlatformWeb` leaf in the wizard detail pane: add a caption and ensure the
guided-command block (with the `c`-copy hint) is the sole call-to-action when the Web leaf has guided
commands — i.e. suppress the "coming soon" placeholder hint so there is no dual-CTA. Web stays
non-executable.

**Depends on**: Task 03 (the `PlatformWeb` leaf must carry real `guided_commands` for the rendering to be
meaningful and for the suppression test to be exercised).

**Agent:** implementor

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` — `step_caption`, `render_action_hint`;
  tests.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/types.rs` — `WizardStepKind`.
- `crates/fdemon-app/src/install_wizard/state.rs` — `WizardStep` / `GuidedCommand` shape.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name/variant.

#### 1. `step_caption` — Web caption

- `step_caption(kind)` (`:90-103`) — add
  `WizardStepKind::PlatformWeb => Some("  Browser required for flutter run -d chrome")` (or similar
  one-line caption). Today only `PlatformAndroid` and `Prerequisites` return `Some`; the rest return
  `None` via the wildcard.
- The caption height is reserved automatically by `guided_section_full_height` (it counts the caption row
  the same way it counts the JDK caption) — **no separate height change**. Tests that assert rendered row
  counts must account for the extra row.

#### 2. `render_action_hint` — suppress dual-CTA

- The "coming soon" / placeholder branch is skipped when a step both *has guided commands* and is one of
  the explicitly-listed guided kinds. The skip condition (`:264-272`) currently matches
  `WizardStepKind::PlatformAndroid | WizardStepKind::Prerequisites`. Add `WizardStepKind::PlatformWeb`:

  ```rust
  matches!(kind,
      WizardStepKind::PlatformAndroid
      | WizardStepKind::Prerequisites
      | WizardStepKind::PlatformWeb)
  ```

  Without this, a Web leaf with guided commands renders the guided-command block **and** the "coming soon"
  text simultaneously (the most likely rendering regression — verified gotcha).

#### 3. `is_executable` — NO CHANGE

- `is_executable(kind, has_guided_commands)` (`:218-224`) — leave unchanged. Web is guided-only; the
  wildcard `_ => false` is correct. **Do not** add `PlatformWeb` here.

### Acceptance Criteria

1. The Web leaf detail pane renders its caption above the guided-command block when guided commands exist.
2. The guided-command block renders with the `c`-copy hint (existing block logic — no new code).
3. The "coming soon" placeholder hint is **not** rendered for `PlatformWeb` when it has guided commands.
4. When the Web leaf has **no** guided commands (browser detected), the pane renders cleanly (no dual-CTA,
   no stray caption issue) — match the existing Prerequisites-with-no-commands behaviour.
5. `cargo test -p fdemon-tui --lib` green; `cargo fmt --all` + `cargo clippy -p fdemon-tui -- -D warnings` clean.

### Testing

```bash
cargo build -p fdemon-tui
cargo test -p fdemon-tui --lib install_wizard
cargo test -p fdemon-tui --lib
cargo fmt --all && cargo clippy -p fdemon-tui -- -D warnings
```

New tests to add:
- `test_step_caption_web_returns_some` — `step_caption(WizardStepKind::PlatformWeb)` is `Some(_)`.
- `test_step_detail_suppresses_coming_soon_for_web_with_guided_commands` — a `PlatformWeb` step with ≥1
  guided command does **not** render the "coming soon" text (mirror the existing Prerequisites
  suppression test).
- A test that a `PlatformWeb` step with **no** guided commands renders without a dual-CTA.

### Notes

- This is the smallest task and has no write-file overlap with any other Phase-3 task.
- The guided-command block itself (label / command / note / `c`-copy hint) already exists from Phase 3's
  Android work — this task only adds the caption and the suppression guard for the Web kind.
- Keep the caption concise; it shares the detail pane with the guided-command block and component rows.
