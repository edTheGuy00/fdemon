## Task: Stub `docs/MOUSE.md` and link it from `docs/CONFIGURATION.md`

**Objective**: Create `docs/MOUSE.md` covering the per-mode modifier behavior, the coordinate-free routing decision, and the Win11 Shift-mod drop caveat. Link it from the `enable_mouse` row in `docs/CONFIGURATION.md` so users discover it. This is the load-bearing user-facing doc that mitigates three medium risks called out in the Phase 2 review.

**Depends on**: None

**Estimated Time**: 0.75h

### Scope

**Files Modified (Write):**
- `docs/MOUSE.md` (NEW): Stub doc with three sections — Modifier Behavior by Mode, Coordinate-Free Scroll, Platform Caveats (Windows 11 Shift-drop). Approximately 80–120 lines.
- `docs/CONFIGURATION.md`: Add a `→ See [Mouse Interaction Reference](MOUSE.md)` link to the `enable_mouse` row's description and/or the "When to disable mouse capture" callout.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/mouse/normal.rs`, `devtools.rs`, `settings.rs`, `new_session.rs`, `link_highlight.rs`, `flutter_version.rs`: Per-mode behavior to document (Up/Down navigation, page-step support, ignored modifiers).
- `workflow/plans/features/mouse-support/PLAN.md`: "Edge Cases" section (lines 353-390 per review) for crossterm #986 (Win11 Shift drop) wording.
- `workflow/reviews/features/mouse-support-phase-2-scroll-wheel/REVIEW.md`: Risks summary that this doc is mitigating.

### Details

#### `docs/MOUSE.md` structure

Suggested skeleton (adjust prose to match other `docs/*.md` voice):

```markdown
# Mouse Interaction Reference

Flutter Demon supports mouse interaction in the terminal when `[ui] enable_mouse = true`
(the default). This document describes how the wheel scrolls each UI mode, the modifier
keys that change scroll behavior, and the platform caveats.

For the on/off setting, see `[ui] enable_mouse` in [CONFIGURATION.md](CONFIGURATION.md).

## Scroll Behavior by UI Mode

Wheel events route to the focused surface based on the current `UiMode`. There is **no
coordinate-based routing** — scrolling anywhere in the terminal scrolls the focused
surface (the log view, the settings list, the active DevTools panel, etc.).

| Mode                    | Plain Wheel                          | Shift+Wheel                       |
|-------------------------|--------------------------------------|-----------------------------------|
| Normal (logs)           | Scroll log line up/down              | Page log up/down                  |
| Normal (tag-filter on)  | Move tag-filter selection            | Move tag-filter selection         |
| LinkHighlight           | Scroll log line up/down              | Page log up/down                  |
| DevTools — Inspector    | Tree row up/down                     | (no-op)                           |
| DevTools — Performance  | (no-op — keyboard ←/→ for frames)    | (no-op)                           |
| DevTools — Network      | Request list up/down                 | Page request list up/down         |
| Settings                | Item selection up/down               | Item selection up/down            |
| Settings (modal)        | Modal item up/down                   | Modal item up/down                |
| NewSessionDialog        | Device / field up/down               | Device / field up/down            |
| FlutterVersion          | Version selection up/down            | Version selection up/down         |
| SearchInput, Confirm,   | (no-op — text input or modal)        | (no-op)                           |
| Loading, EmulatorSelector |                                    |                                   |

`Ctrl+Wheel` and `Alt+Wheel` are reserved (no-op) in modes that honor `Shift+Wheel` —
this avoids conflict with terminal-level `Ctrl+Wheel` font-zoom bindings. In modes that
already ignore modifiers (Settings, NewSessionDialog, FlutterVersion), `Ctrl`/`Alt` are
simply passed through and produce the same single-step navigation as plain wheel.

Horizontal scroll (`ScrollDir::Left`/`Right` from touchpads) is currently a no-op in all
modes. Future phases may map horizontal scroll to log timeline panning or DevTools
secondary-axis navigation.

## Coordinate-Free Routing

Wheel events are routed by `UiMode` only — the cursor position `(x, y)` does not affect
which surface receives the scroll. This means scrolling while hovering over the header,
status bar, or session tabs still scrolls the focused surface (e.g., the log view in
`Normal` mode).

This is a deliberate v1 simplification. A future phase will introduce region-based
hit-testing, at which point scroll routing may also become coordinate-aware.

## Platform Caveats

### Windows 11 — Shift modifier dropped on mouse events

Crossterm issue [#986](https://github.com/crossterm-rs/crossterm/issues/986) documents
that Windows 11 (running under modern Windows Terminal or conhost) drops the Shift
modifier on mouse events before crossterm can read it. The practical impact:

- `Shift+Wheel` degrades to plain wheel in Normal, LinkHighlight, and DevTools/Network.
- Page-step scrolling via the wheel is therefore unavailable on Windows 11.
- Workaround: use the keyboard `PageUp`/`PageDown` keys, which are unaffected.

Other platforms (macOS, Linux, older Windows builds) are not affected.

### Legacy Windows conhost — mouse capture silently ignored

If your terminal is the legacy `conhost.exe` shipped with pre-Windows-10 Windows, mouse
capture escape sequences are silently ignored and the wheel is never delivered to fdemon.
Set `enable_mouse = false` in `.fdemon/config.toml` to opt out cleanly; otherwise wheel
events fall through to the host terminal's scrollback (which is the desired behavior
when capture doesn't work).

## Disabling Mouse Capture

If you prefer wheel events to drive your terminal's native scrollback (or you are on
legacy Windows conhost), set:

```toml
[ui]
enable_mouse = false
```

Restart fdemon after changing this setting. See `[ui] enable_mouse` in
[CONFIGURATION.md](CONFIGURATION.md) for details.

## Future Work

- Coordinate-aware click handling (region registry, header shortcuts, log-row clicks)
- Drag-to-select for log lines
- Horizontal-scroll consumers
- First-launch hint for users who didn't realize mouse is captured
```

The exact wording can vary; the load-bearing requirement is the three sections (per-mode behavior, coordinate-free explanation, Windows 11 caveat).

#### `docs/CONFIGURATION.md` link

Find the `enable_mouse` row added by Phase 1.5 Task 05 (around line 328 of `docs/CONFIGURATION.md`) and append a sentence to the description, or modify the "When to disable mouse capture" callout that follows the table. Suggested edit to the description column:

```markdown
| `enable_mouse` | `boolean` | `true` | Enables terminal mouse capture for clickable UI elements (header shortcuts, tabs, log view, DevTools panels). When `false`, fdemon does not emit mouse-capture escape sequences, leaving native terminal behavior (text selection, wheel scrollback) intact. **Restart required after changing.** See [MOUSE.md](MOUSE.md) for per-mode wheel behavior, modifier reference, and platform caveats. |
```

Or, if you prefer the link in the callout, add a final sentence:

```markdown
> **When to disable mouse capture:** ... [existing text] ...
> See [MOUSE.md](MOUSE.md) for the full per-mode wheel reference and Windows 11 caveat.
```

Either placement is acceptable; pick whichever reads better in context.

### Acceptance Criteria

1. `docs/MOUSE.md` exists and contains three top-level sections: per-mode scroll behavior, coordinate-free routing explanation, platform caveats (with explicit Windows 11 + crossterm #986 reference).
2. The per-mode table documents at minimum: Normal (with tag-filter on/off), LinkHighlight, all three DevTools panels, Settings, NewSessionDialog, FlutterVersion, and the four no-op modes.
3. The Windows 11 Shift-drop caveat names crossterm issue #986 (the link itself can be inline or a footnote).
4. `docs/CONFIGURATION.md`'s `enable_mouse` row OR the adjacent callout links to `docs/MOUSE.md`.
5. Markdown renders cleanly (no broken table syntax, no broken links).
6. No code files are touched.

### Testing

Visual review only. Optional:

```bash
# Render in a markdown previewer (grip, glow, vscode preview, etc.)
grip docs/MOUSE.md
grip docs/CONFIGURATION.md
```

Check that the cross-link from CONFIGURATION.md → MOUSE.md and back resolves.

### Notes

- **`docs/MOUSE.md` is unmanaged** — it is not subject to `doc_maintainer` content boundaries (those apply to `ARCHITECTURE.md`, `CODE_STANDARDS.md`, `DEVELOPMENT.md`). The default implementor agent owns this task.
- **Phase 6 of the parent feature plan** (`workflow/plans/features/mouse-support/PLAN.md`) is scheduled to expand this doc with click handling, drag selection, etc. The Phase 2.5 stub establishes the file and answers the three immediate user-facing questions; Phase 6 fleshes out the rest.
- **Why now and not Phase 6.** Three medium-severity risks from the Phase 2 review (Win11 Shift drop, modifier asymmetry, coordinate-free routing) cite `docs/MOUSE.md` as the mitigation. Without the file, users on `main` between Phase 2 ship and Phase 6 ship have nothing to consult. The stub is cheap insurance.
- **Wording style** — match the voice of existing `docs/*.md` files (especially `docs/CONFIGURATION.md`, which is the nearest neighbor in tone). Tables and `>` callouts are used freely.
- **DO NOT alter `crates/fdemon-app/src/handler/mouse/*.rs`.** This is a docs-only task. Pull behavior facts from the source code and the parent PLAN.md but do not edit code.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `docs/MOUSE.md` | NEW: per-mode scroll reference, coordinate-free explanation, platform caveats |
| `docs/CONFIGURATION.md` | Added link to `MOUSE.md` from `enable_mouse` row AND from the "When to disable mouse capture" callout |

### Notable Decisions/Tradeoffs

1. **Link in two places in CONFIGURATION.md**: Added the link both to the `enable_mouse` table row description and to the "When to disable mouse capture" callout. The task allowed either placement; both placements maximizes discoverability since users reading either section are taken to the full reference.
2. **Inspector modifier note**: Added a callout in MOUSE.md explaining the `Shift+Ctrl+Wheel` single-step behavior (rather than no-op), matching the actual implementation in `devtools.rs::handle_inspector_scroll`. This directly documents the known inconsistency flagged by the Phase 2 review.
3. **DevTools/Network filter-input mode**: Added a separate table row for the filter-input-active case, which is a meaningful behavior distinction (all scroll swallowed) confirmed in `devtools.rs`.

### Testing Performed

- Visual markdown review — Passed
- Link resolution between CONFIGURATION.md and MOUSE.md — Passed (both files in same `docs/` directory; relative links resolve)
- All three acceptance-criteria sections present — Passed
- crossterm #986 reference confirmed present — Passed
- No code files modified — Confirmed

### Risks/Limitations

1. **Stub is intentionally narrow.** Click handling, drag selection, region behavior are not documented yet — those land in Phase 6.
