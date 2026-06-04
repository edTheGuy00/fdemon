## Task: Extract a single `step_caption` helper for the guided section (F3)

**Severity:** MINOR

**Objective**: Eliminate the duplicated caption derivation between
`guided_section_full_height` and `render_guided_commands` in `step_detail.rs`, so a
future captioned step kind cannot silently desync reserved height from rendered rows
(re-introducing an M1-style clip).

**Depends on**: None

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`

**Files Read (Dependencies):**
- `fdemon-app::install_wizard` re-exports: `WizardStepKind`.

### Details

`guided_section_full_height` (`step_detail.rs:267-295`) derives whether the step has a
caption via:

```rust
let has_caption = matches!(step_kind, WizardStepKind::AndroidTools | WizardStepKind::Prerequisites);
```

while `render_guided_commands` (`:346-369`) independently derives the caption **text**:

```rust
let caption_text = match step_kind {
    WizardStepKind::AndroidTools  => Some("  JDK 17 required before installing Android tools"),
    WizardStepKind::Prerequisites => Some("  Install the OS build tools below, then press r to re-check"),
    _ => None,
};
let has_caption = caption_text.is_some();
```

These two sites must agree on the captioned-step-kind set forever. They match today
(no live bug — confirmed by all three logic/quality/risks reviewers), but the
duplication is a latent drift hazard: a new captioned step kind added to only one site
makes the height reservation and the rendered rows disagree, clipping a command.

**Fix:** Introduce one source of truth — a function returning the caption text (the
`has_caption` boolean is then just `.is_some()`):

```rust
/// The per-step caption rendered above the guided-command list, if any.
/// Single source of truth for both the height reservation
/// (`guided_section_full_height`) and the renderer (`render_guided_commands`),
/// so the two can never disagree on which steps have a caption.
fn step_caption(kind: WizardStepKind) -> Option<&'static str> {
    match kind {
        WizardStepKind::AndroidTools  => Some("  JDK 17 required before installing Android tools"),
        WizardStepKind::Prerequisites => Some("  Install the OS build tools below, then press r to re-check"),
        _ => None,
    }
}
```

Then:
- `guided_section_full_height`: `let has_caption = step_caption(step_kind).is_some();`
- `render_guided_commands`: `let caption_text = step_caption(step_kind); let has_caption = caption_text.is_some();`

Keep the exact caption strings byte-for-byte. This is a pure refactor — no behavior change.

### Acceptance Criteria

1. One `step_caption(kind) -> Option<&'static str>` (or equivalent single function) is
   the sole place the captioned-step-kind set and caption strings are defined.
2. Both `guided_section_full_height` and `render_guided_commands` call it; neither
   contains its own `matches!(kind, AndroidTools | Prerequisites)` or inline caption
   `match`.
3. No behavior change: caption strings, `has_caption` semantics, and all existing render
   output are identical.
4. Existing `guided_section_full_height` unit tests (e.g. `=12` for 3 commands with
   caption) and all `step_detail` render tests stay green.

### Testing

```rust
#[cfg(test)]
mod tests {
    // - existing guided_section_full_height + render tests remain green (the refactor
    //   is behavior-preserving).
    // - optional: a direct test of step_caption(AndroidTools)/step_caption(Prerequisites)
    //   returning Some, and step_caption(FlutterSdk/PathConfig/Doctor) returning None.
}
```

### Notes

- Pure refactor; no production behavior should change. This is the clean base that task
  02 (F1 scroll window) builds on.
- This task only touches `step_detail.rs`; it is parallel-safe with tasks 04 and 05.
