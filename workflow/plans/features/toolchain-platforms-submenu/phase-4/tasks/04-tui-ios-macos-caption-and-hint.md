## Task: TUI — iOS/macOS leaf caption + "coming soon" suppression

**Objective**: Render the `PlatformIos` and `PlatformMacos` leaves in the install-wizard detail pane like
the live `PlatformWeb` leaf: a yellow caption above the guided-command block, and suppression of the
"coming soon" hint when the leaf has guided commands. Exactly two edits in `step_detail.rs`; `step_list.rs`
and `mod.rs` need no changes (they are data-driven).

**Depends on**: Task 03 (merged) — leaves must carry real captions/guided commands for meaningful tests.

**Agent:** implementor

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` — `step_caption()` arms +
  `render_action_hint()` `matches!` allowlist; add render tests.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/{types,state}.rs` — `WizardStepKind`, `WizardStep`,
  `GuidedCommand`.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/variant.

#### Edit 1 — `step_caption()` (the single source of truth for both height + render)

Add iOS/macOS arms beside the existing `PlatformWeb` arm:

```rust
fn step_caption(kind: WizardStepKind) -> Option<&'static str> {
    match kind {
        WizardStepKind::PlatformAndroid => Some("  JDK 17 required before installing Android tools"),
        WizardStepKind::Prerequisites => Some("  Install the OS build tools below, then press r to re-check"),
        WizardStepKind::PlatformWeb => Some("  Browser required for flutter run -d chrome"),
        WizardStepKind::PlatformIos => Some("  Xcode required for iOS development"),
        WizardStepKind::PlatformMacos => Some("  Xcode required for macOS development"),
        _ => None,
    }
}
```

#### Edit 2 — `render_action_hint()` "coming soon" suppression allowlist

Add `PlatformIos` and `PlatformMacos` to the `matches!()` pattern that suppresses the "coming soon" hint
when guided commands are present:

```rust
} else if has_guided_commands
    && matches!(
        kind,
        WizardStepKind::PlatformAndroid
            | WizardStepKind::Prerequisites
            | WizardStepKind::PlatformWeb
            | WizardStepKind::PlatformIos      // NEW
            | WizardStepKind::PlatformMacos    // NEW
    )
{
    return;
}
```

#### No other changes

- **`is_executable()`** — `_ => false` already covers iOS/macOS (guided-only, never auto-executable). No
  new arm. Confirm it returns `false` for both (so the Enter-install hint never renders).
- **`action_hint_text()`** — never reached for non-executable kinds; `_ => ""` covers them. No change.
- **`step_list.rs`** — indent (`step.indent`), caret (`kind == Platforms`), run-failed badge, and the
  `StepStatus` glyph match are all data-driven. **Zero changes.**
- **`mod.rs`** — step-list height is dynamic (`self.state.steps.len()`); the footer hint only special-
  cases the `Platforms` parent. **Zero changes.**

#### Behaviour matrix (mirror `PlatformWeb`)

| Leaf state | guided_commands | Detail pane |
|------------|-----------------|-------------|
| Xcode/CocoaPods absent (`Partial`) | non-empty | caption + guided block (`c` to copy); **no** "coming soon" |
| Xcode/CocoaPods all `Ok` | empty | caption + status; "coming soon" hint renders (display-only) |

### Acceptance Criteria

1. The iOS leaf renders `"Xcode required for iOS development"` and the macOS leaf renders `"Xcode required
   for macOS development"` as the caption above the guided-command block.
2. When the leaf has guided commands, the "coming soon" hint is **suppressed** (no dual-CTA); the guided
   block + `c`-copy hint render.
3. When the leaf has no guided commands (Xcode all `Ok`), the "coming soon" hint **renders** (display-only).
4. `is_executable()` returns `false` for both leaves (no Enter-install hint).
5. No panic at small terminal sizes; `cargo test -p fdemon-tui --lib` green; `cargo fmt --all` +
   `cargo clippy -p fdemon-tui -- -D warnings` clean.

### Testing

Mirror `test_step_detail_shows_guided_block_for_prerequisites_step_with_commands` and the
"coming soon" counterpart:
- `test_ios_leaf_shows_caption_and_guided_block` — `PlatformIos` with guided commands → caption present,
  guided block present, "coming soon" absent.
- `test_macos_leaf_shows_caption_and_guided_block` — same for `PlatformMacos`.
- `test_ios_leaf_no_commands_shows_coming_soon` — `PlatformIos` with empty `guided_commands` → caption
  present, "coming soon" hint present.
- `test_ios_macos_leaves_not_executable` — assert no "Press Enter to install" hint renders.
- No-panic render tests at minimal `Rect` sizes.

### Notes

- `step_caption()` is shared by `guided_section_full_height()` and `render_guided_commands()` — adding the
  arm once keeps height and render in sync automatically.
- Keep the captions `&'static str` to match the function signature.

---

## Completion Summary

**Status:** Not Started
