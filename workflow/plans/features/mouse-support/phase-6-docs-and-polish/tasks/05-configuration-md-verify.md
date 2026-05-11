## Task: CONFIGURATION.md `enable_mouse` Verify-and-Update

**Objective**: Verify the existing `[ui] enable_mouse` documentation in `docs/CONFIGURATION.md` (lines 316, 328, 334) accurately describes Phase 5/5.5 final runtime behaviour. Correct any drift; otherwise mark the task complete with a no-write outcome.

**Depends on**: None

**Estimated Time**: 0.25h

### Scope

**Files Modified (Write):**
- `docs/CONFIGURATION.md`: Edit only if drift is found. Likely no-write task.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs` — `UiSettings::enable_mouse` field, default value, deserialization.
- `crates/fdemon-tui/src/terminal.rs` — `enable_mouse_capture()` / `disable_mouse_capture()` helpers; `MOUSE_CAPTURE_ON` AtomicBool.
- `crates/fdemon-tui/src/runner.rs` — call sites for enable/disable in entry paths and panic hook.
- `docs/CONFIGURATION.md` lines 316, 328, 334 — current `enable_mouse` documentation.

### Details

The current CONFIGURATION.md content (verified at the time of plan authoring):

- Line 316 (TOML example block): `enable_mouse = true             # Capture mouse events for clickable UI; restart required`
- Line 328 (settings reference table row): `| ` + `enable_mouse` + ` | boolean | true | Enables terminal mouse capture for clickable UI elements (header shortcuts, tabs, log view, DevTools panels). When false, fdemon does not emit mouse-capture escape sequences, leaving native terminal behavior (text selection, wheel scrollback) intact. **Restart required after changing.** See [MOUSE.md](MOUSE.md) for per-mode wheel behavior, modifier reference, and platform caveats. |`
- Line 334 (callout block): "When to disable mouse capture" — recommends disabling on legacy Windows conhost, terminals without `Shift+drag` selection, or for users who find wheel-intercept-vs-host-scrollback disorienting. Cross-references MOUSE.md for the Windows 11 Shift-drop caveat.

Verify each statement against current source:

1. **Default is `true`** — confirm `crates/fdemon-app/src/config/types.rs::UiSettings` has `enable_mouse: bool` defaulted to `true` (likely via a `default_true_for_mouse()` helper to make explicit `false` survive round-trip).
2. **"Restart required after changing"** — confirm the runner reads the setting once at startup and does not re-evaluate it. Check `runner.rs` and any settings-mutation paths.
3. **"clickable UI elements (header shortcuts, tabs, log view, DevTools panels)"** — Phase 5 added dialogs + overlays + Settings + LinkHighlight badges. The list at line 328 is incomplete. Either expand the parenthetical OR replace it with a more general phrase + cross-reference to MOUSE.md.
4. **"native terminal behavior (text selection, wheel scrollback)"** — confirm this remains accurate (it should — `enable_mouse = false` skips both `EnableMouseCapture` and the disable; the AtomicBool guard ensures no escape sequences are written).
5. **Cross-link `[MOUSE.md](MOUSE.md)`** — confirm the path is correct (relative, no anchor, matches the file's location).

If any of the above is wrong or stale, edit the affected line(s). If everything is current, this task closes with no edits.

### Acceptance Criteria

1. Each of the five verification points above is confirmed against current source. The verifier records the result in the completion summary (one sentence per point: "verified" or "drift found, fixed by ...").
2. If line 328's parenthetical list is found to be stale (it is — Phase 5 added dialogs/overlays), it is updated to either the expanded list (`header shortcuts, tabs, log view, DevTools panels, dialogs, overlays, Settings panel`) OR replaced with a generic "clickable UI elements throughout the TUI; see [MOUSE.md](MOUSE.md) for the full surface map."
3. No other CONFIGURATION.md content is touched. No other settings rows, no formatting changes elsewhere.
4. `grep "enable_mouse" docs/CONFIGURATION.md` returns the same number of matches before and after (no accidental duplication or removal).

### Testing

```bash
# Verify default value:
grep -n "enable_mouse" crates/fdemon-app/src/config/types.rs

# Verify call sites:
grep -n "enable_mouse_capture\|disable_mouse_capture" crates/fdemon-tui/src/

# Re-render and read line 328:
grep -A 1 "enable_mouse" docs/CONFIGURATION.md
```

### Notes

- This task is intentionally lightweight. The CONFIGURATION.md row was authored during Phase 1 and re-checked during Phase 5; it is likely already accurate. The parenthetical list is the most likely drift point.
- If you find that the runtime behaviour differs from what CONFIGURATION.md claims (e.g. the AtomicBool guard is broken, or restart is no longer required), do not paper over it in docs — file a defect and pause the task.
- Do not edit the "When to disable mouse capture" callout unless its content is materially wrong. Phrasing improvements are out of scope.
- This task does **not** edit MOUSE.md, ARCHITECTURE.md, or any other doc.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Line 328: replaced stale parenthetical list with generic phrase + MOUSE.md cross-link |

### Notable Decisions/Tradeoffs

1. **Generic phrase chosen over expanded list**: The task offered two options for fixing the stale parenthetical. Used "throughout the TUI (see [MOUSE.md](MOUSE.md) for the full surface map)" rather than an expanded itemised list, so future phases adding new clickable surfaces will not require another CONFIGURATION.md edit.

### Verification Points (all five from task spec)

1. **Default is `true`** — Verified. `crates/fdemon-app/src/config/types.rs` line 300 sets `enable_mouse: true` in `Default::default()`, and line 286 uses `#[serde(default = "default_true")]` for explicit-`false` round-trip safety.
2. **"Restart required after changing"** — Verified. `runner.rs` reads `engine.settings.ui.enable_mouse` once at startup (lines 34 and 127). No dynamic re-evaluation path exists; restart is genuinely required.
3. **Parenthetical list stale** — Drift found and fixed. Phase 5 added dialogs, overlays, Settings panel, and LinkHighlight badges; the old list "(header shortcuts, tabs, log view, DevTools panels)" was incomplete. Replaced with a generic description.
4. **"native terminal behavior (text selection, wheel scrollback)"** — Verified. The `MOUSE_CAPTURE_ON` AtomicBool guard ensures no mouse-capture or disable escape sequences are written when `enable_mouse = false`; native behaviour is preserved as documented.
5. **Cross-link `[MOUSE.md](MOUSE.md)`** — Verified. `docs/MOUSE.md` exists; the relative path is correct for a link from `docs/CONFIGURATION.md`.

### Testing Performed

- `grep -n "enable_mouse" crates/fdemon-app/src/config/types.rs` — Confirmed default `true` at lines 287/300 with `default_true` serde helper
- `grep -n "enable_mouse_capture\|disable_mouse_capture" crates/fdemon-tui/src/terminal.rs` — Confirmed AtomicBool guard present
- `grep -n "enable_mouse" crates/fdemon-tui/src/runner.rs` — Confirmed single read-at-startup pattern (lines 34, 127)
- `grep -n "enable_mouse" docs/CONFIGURATION.md` — 3 matches before and after edit (no duplication or removal)

### Risks/Limitations

1. **No code changes**: This task is docs-only; no functional risk.
