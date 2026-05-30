## Task: Spinner icon for the launch-lifecycle phases in the status bar

**Objective**: In the bottom status bar, render the existing braille spinner
(Phase 3) in place of the static `○` icon for the launch-lifecycle phases
(`Initializing`, `Preparing`, `Launching`) so the in-progress glyph animates in
unison with the new-session dialog's discovery spinner. All other phases keep their
static `phase_indicator` icon. Scope is the **bottom status bar only** — the header
title row and session tabs are unchanged.

**Depends on**: none

**Estimated Time**: 0.5–1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: in `render_bottom_metadata`,
  choose a spinner glyph instead of the static `icon` when `status.phase` is a
  launch-lifecycle phase; add a unit test.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/spinner.rs`: `spinner_char`, `SPINNER_TICKS_PER_FRAME`
  (exist since Phase 3 — import, do **not** modify).
- `crates/fdemon-core` (`AppPhase`): the phase variants to match on.
- `crates/fdemon-tui/src/theme/styles.rs`: `phase_indicator` / `phase_indicator_busy`
  (existing — still supplies the color/label/static icon).

### Background

`render_bottom_metadata` (`log_view/mod.rs:885`) currently builds the left side as a
**static** icon plus a shimmered/static label:

```rust
let (icon, label, phase_style) = if status.is_busy {
    theme_styles::phase_indicator_busy(icons)
} else {
    theme_styles::phase_indicator(status.phase, icons)
};
// ...
let mut spans = vec![
    Span::raw(" "),
    Span::styled(icon, phase_style),   // <-- always static
    Span::raw(" "),
];
spans.extend(Self::status_label_spans_inner(label, phase_style, is_transient, status.animation_frame));
```

The `phase_indicator` mapping (`theme/styles.rs:130`) gives `Initializing`,
`Preparing`, and `Launching` the same `○` (`icons.circle()`) glyph — there is no
distinctive launch glyph to preserve, which is why these three (and only these
three) get the spinner. `Reloading` keeps `↻`, `Quitting` keeps `✗`, `Running` keeps
`●`, `Stopped` keeps `○`.

The spinner cadence must match the dialog (`target_selector.rs:331`,
`tab_bar.rs:113`): `spinner_char(status.animation_frame / SPINNER_TICKS_PER_FRAME)`.
`status.animation_frame` is the global `AppState::animation_frame` already plumbed
into `StatusInfo`, so all spinners pulse in unison.

### Details

**1. Decide the leading glyph.** After computing `(icon, label, phase_style)`, derive
the glyph to render:

```rust
use crate::widgets::spinner::{spinner_char, SPINNER_TICKS_PER_FRAME};

// Launch-lifecycle phases animate their glyph; every other phase (incl. the
// is_busy / Reloading path) keeps its static phase_indicator icon.
let is_launch_phase = !status.is_busy
    && matches!(
        status.phase,
        AppPhase::Initializing | AppPhase::Preparing | AppPhase::Launching
    );

let icon_span = if is_launch_phase {
    let glyph = spinner_char(status.animation_frame / SPINNER_TICKS_PER_FRAME);
    Span::styled(glyph.to_string(), phase_style)
} else {
    Span::styled(icon, phase_style)
};
```

Then use `icon_span` in place of `Span::styled(icon, phase_style)` in the `spans`
vec. Keep the surrounding `Span::raw(" ")` padding and the label/shimmer logic
exactly as-is.

- Gate on `!status.is_busy` so the `phase_indicator_busy` (`Reloading`) path is never
  spinner-ized — `Reloading` keeps `↻`.
- `spinner_char` returns a `char`; convert with `.to_string()` since the static
  `icon` branch yields a `&'static str` and the spans must share a type. (A small
  `String` per render is fine — matches the dialog call sites.)

**2. Keep the label untouched.** `is_transient` and `status_label_spans_inner` are
unchanged — `Initializing`/`Preparing`/`Launching` already shimmer their label; now
their leading glyph spins too.

### Acceptance Criteria

1. With `status.phase` ∈ {`Initializing`, `Preparing`, `Launching`} (and
   `is_busy == false`), the leading glyph is a `SPINNER_FRAMES` braille character
   selected by `animation_frame / SPINNER_TICKS_PER_FRAME`, styled with the phase's
   `phase_style` color.
2. With `status.phase` ∈ {`Reloading`, `Quitting`, `Running`, `Stopped`}, or whenever
   `status.is_busy` is true, the leading glyph is exactly the static
   `phase_indicator` / `phase_indicator_busy` icon (`↻`, `✗`, `●`, `○`) — unchanged.
3. The spinner advances at the same cadence as the new-session dialog spinner (same
   `SPINNER_TICKS_PER_FRAME` divisor, same global `animation_frame`), so concurrent
   spinners are in phase.
4. The status **label** shimmer behaviour is unchanged for all phases.
5. The header title row and session tabs still render their static phase icons (this
   task does not touch `header.rs` or `tabs.rs`).
6. `cargo test -p fdemon-tui --lib`, `cargo fmt`, and `clippy` are clean.

### Testing

Add a unit test in `log_view/mod.rs`'s test module (model after existing
`render_bottom_metadata` / status tests that build a `StatusInfo` and render into a
`Buffer`):

```rust
#[test]
fn launch_phases_show_spinner_glyph() {
    // StatusInfo { phase: Launching, is_busy: false, animation_frame: F, .. }
    // → first non-space glyph cell is a SPINNER_FRAMES char (e.g.
    //   spinner_char(F / SPINNER_TICKS_PER_FRAME)), not '○'.
}

#[test]
fn non_launch_phases_keep_static_icon() {
    // phase: Reloading → glyph is '↻'; phase: Running → '●'; phase: Stopped → '○'.
}

#[test]
fn launch_spinner_advances_with_frame() {
    // Two renders with animation_frame F and F + SPINNER_TICKS_PER_FRAME yield
    // different spinner glyphs (proves it animates off the global frame).
}
```

Inspect the rendered `Buffer` cell where the leading glyph lands (after the leading
`Span::raw(" ")`). Use the NerdFonts-vs-Unicode `IconSet` consistently with how the
existing status tests construct `icons`.

### Notes

- **Why only these three phases.** `Initializing`/`Preparing`/`Launching` all map to
  the same featureless `○`; a spinner communicates "working" far better. `Reloading`
  (`↻`) and `Quitting` (`✗`) have meaningful glyphs and are intentionally left static
  (confirmed with the user).
- **Status bar only.** Per the confirmed decision, the header and tabs keep static
  icons to avoid many simultaneous spinners across up to 9 tabs plus the header.
- **No new state.** `status.animation_frame` already carries the global frame; this
  task only changes which glyph is chosen at render time.
- **No managed-doc change.** No `AppPhase`/`Message`/module change — no
  architecture/standards/dev-doc update required.
