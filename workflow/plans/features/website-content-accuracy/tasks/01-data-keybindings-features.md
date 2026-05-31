## Task: Fix keybindings + features data, add multi-launch keys

**Objective**: Make `all_keybinding_sections()` and `features()` in `data.rs` match the
real key handling, and add the shipped multi-device launch keys.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `website/src/data.rs`: add missing keybindings/modes, fix mappings, add a Multi-Device
  Launch section, refresh `features()` copy.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/keys.rs`: source of truth for key bindings.
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`,
  `crates/fdemon-app/src/handler/new_session/target_selector.rs`: multi-launch behavior.
- `crates/fdemon-app/src/session_manager.rs:12`: `MAX_SESSIONS = 9`.
- `docs/KEYBINDINGS.md`: canonical key reference.

### Details

**Add missing keybindings (HIGH):**
- `Alt+m` / `Alt+M` — Toggle mouse capture (global, all modes). [D-01] (`keys.rs:29`)
- `D` — Toggle DAP debug-adapter server. [D-02] (`keys.rs:246`)
- `V` — Open Flutter SDK version manager panel. [D-03] (`keys.rs:355`)
- `w` — Toggle log wrap mode. [D-04] (`keys.rs:330`)
- Note `t` (lowercase) also toggles the tag-filter overlay alongside `T`. [D-05]

**Add a "Multi-Device Launch" subsection** (new `KeybindingSection`) — new-session dialog
(Connected tab) multi-select:
- `Space` — toggle the cursor device's selection.
- `a` — select all supported devices / clear all if already all-selected.
- `Enter` — launch all checked devices (or just the cursor device if none checked).
- `r` — refresh device list.
- Mention the footer hint `Space select · a all · Enter launch · r refresh`, the
  `(N selected)` counter, and that unsupported devices can't be selected.

**Add a Flutter Version mode section** [D-10]: `Esc`, `Tab`, `k`/`↑`, `j`/`↓`, `Enter`,
`d`, `i`, `u` (`keys.rs:376-399`, `docs/KEYBINDINGS.md:585-614`).

**Add a Loading mode section** [D-11]: `q`, `Esc`, `Ctrl+C` (`keys.rs:93-99`).

**Fix DevTools keybinding sections:**
- Add `m` — Memory panel to Panel Navigation. [D-06] (`keys.rs:878`)
- Move the `s` allocation-sort binding from Performance → Memory; real guard is
  `in_memory` (`keys.rs:1012`). [D-07]
- Complete the Performance panel bindings [D-09]: `Tab`/`Shift+Tab`, `j/k`/`↑↓`,
  `PageUp/PageDown`, `Home/End`, `]`/`[` (cycle detail tabs), `f` (filter on Timeline
  Events), `R` (rebuild tracking), `+`/`=` zoom in, `-`/`_` zoom out, `g` follow latest,
  `/` timeline search, `n`/`N` search nav (`keys.rs:585-792`).
- Clarify Performance `←`/`→` are context-dependent (sibling nav / Gantt pan / frame
  select), not simply "prev/next frame". [D-08]

**Fix `features()` copy** [D-12]: keep "Run up to 9 simultaneous sessions" (verified
`MAX_SESSIONS = 9`) and add that one confirm can launch several devices at once. Soften
"monitors your `lib/` directory" to note watcher paths are configurable (default `lib`).

### Acceptance Criteria

1. Every keybinding listed matches a real arm in `keys.rs` (cite line in PR notes).
2. `Alt+m`, `D`, `V`, `w`, `m` (Memory) present; `s` is under Memory, not Performance.
3. Multi-Device Launch, Flutter Version, and Loading sections exist.
4. Performance section lists the full binding set.
5. `features()` multi-launch + watcher copy updated.
6. `cd website && trunk build` compiles.

### Notes

- This file feeds both the Keybindings page and the home-page feature grid; verify both
  render after the change.
- The SEO plan's S09 also edits `data.rs` (landing copy) and must run after this task.
</content>
