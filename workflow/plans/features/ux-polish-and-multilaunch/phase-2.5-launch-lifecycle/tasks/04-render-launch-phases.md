## Task: Render the launch phases — shimmer + progress text

**Objective**: Make `Preparing`/`Launching` shimmer (reuse Phase 2) in the bottom metadata bar and render the live `current_progress` text next to the label. Thread `current_progress` through `StatusInfo`. Steady `Running`/`Stopped`/`Reloading` stay unchanged.

**Depends on**: 02-session-launch-state (the `current_progress` field + new variants)

**Estimated Time**: 1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: add `Preparing`/`Launching` to the `is_transient` match; add a `progress: Option<&str>` field to `StatusInfo`; render the progress suffix.
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`: update existing `StatusInfo { .. }` literals; add shimmer/progress assertions.
- `crates/fdemon-tui/src/render/mod.rs`: populate `StatusInfo.progress` from `handle.session.current_progress`.
- `crates/fdemon-tui/src/render/tests.rs`: snapshot tests for `Launching` and `Preparing`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/session.rs`: `current_progress` (task 02).
- `crates/fdemon-tui/src/widgets/shimmer.rs`: `shimmer_phase`, `shimmer_spans` (Phase 2).
- `crates/fdemon-tui/src/theme/styles.rs`: `phase_indicator` (returns the `STATUS_BLUE` label/style from task 01).

### Details

**1. Shimmer the new phases.** In the transient check (the `is_transient` `matches!` in `render_bottom_metadata`/`status_label_spans_inner`, added in Phase 2 at `log_view/mod.rs` ~line 889), add the two variants:

```rust
let is_transient = status.is_busy
    || matches!(
        status.phase,
        AppPhase::Initializing
            | AppPhase::Preparing
            | AppPhase::Launching
            | AppPhase::Reloading
            | AppPhase::Quitting
    );
```

Because `phase_indicator` (task 01) already returns the `STATUS_BLUE` label, and the label is shimmered using its own fg as the base (Phase 2), the new phases shimmer in blue automatically.

**2. Thread `current_progress` into `StatusInfo`.** Add a borrowed field:

```rust
pub struct StatusInfo<'a> {
    pub phase: &'a AppPhase,
    pub is_busy: bool,
    // … existing fields (incl. animation_frame from Phase 2) …
    /// Live launch progress line (build / pre-app readiness); shown next to
    /// a transient phase label. `None` when nothing is in flight.
    pub progress: Option<&'a str>,
}
```

Populate at the construction site in `render/mod.rs` (next to `animation_frame: state.animation_frame`):

```rust
progress: handle.session.current_progress.as_deref(),
```

**3. Render the progress suffix.** After the (shimmered) label spans, when `is_transient` and `status.progress` is `Some(text)`, append a dim separator + text, e.g. ` · Running Gradle task…`:

```rust
if is_transient {
    if let Some(progress) = status.progress {
        spans.push(Span::styled(
            format!("  {progress}"),
            Style::default().fg(palette::TEXT_MUTED),
        ));
    }
}
```

- Keep the progress text **static** (muted), not shimmered — only the phase label shimmers.
- Mind the 50-line function limit (Phase 2 already extracted `status_label_spans_inner`); keep the addition tight or extend that helper.
- Truncate gracefully if the bar is narrow (reuse any existing truncation helper, or guard on remaining width) so the progress text never overflows the metadata bar.

**4. Steady states unchanged.** `Running`/`Stopped` (not transient) render exactly as before — no progress suffix, no shimmer.

### Acceptance Criteria

1. `StatusInfo` gains `progress: Option<&str>`; `render/mod.rs` passes `handle.session.current_progress.as_deref()`.
2. While `phase ∈ {Preparing, Launching}` (or other transient phases) the label renders as shimmering per-character spans (fg varies across the label).
3. When `progress` is `Some`, a muted, non-shimmered progress suffix is appended after the label in transient phases; when `None`, no suffix.
4. `Running`/`Stopped` render identically to before (no progress suffix, single static label span).
5. The icon glyph is unchanged in all phases (only the label shimmers).
6. All existing `StatusInfo { .. }` literals in `tests.rs` compile (each gains `progress: None` unless asserting progress).
7. New insta snapshots exist for `Launching` and `Preparing`; existing snapshots are unchanged.
8. `cargo test -p fdemon-tui` passes.

### Testing

Add to `log_view/tests.rs`:
- `launching_label_shimmers_across_chars` — `phase=Launching`, `animation_frame` set; label cells do not all share one fg.
- `preparing_label_shimmers_across_chars` — same for `Preparing`.
- `progress_suffix_rendered_when_present` — `phase=Launching`, `progress=Some("Running Gradle task")`; assert the suffix text appears with muted fg.
- `running_has_no_progress_suffix` — `phase=Running`, `progress=Some(..)`; assert no suffix (steady state ignores progress).

Add to `render/tests.rs`: `snapshot_normal_mode_launching` and `snapshot_normal_mode_preparing` (mirror the existing `normal_running`/`normal_initializing` snapshot tests). Run the implementor's test pass to generate the `.snap` files and commit them.

### Notes

- This task touches only `fdemon-tui/*` — **no** shared write files with task 03 (app handlers), so the two run in parallel worktrees.
- Tests here set `current_progress`/`phase` directly on the session (or via `StatusInfo` literals); they do not depend on task 03's population logic.
- Reuse Phase 2's `palette::TEXT_MUTED` / shimmer plumbing; do not introduce a new animation timer.
