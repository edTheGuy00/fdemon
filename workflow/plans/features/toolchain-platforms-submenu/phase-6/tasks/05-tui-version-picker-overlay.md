## Task: TUI — version picker overlay widget

**Objective**: Render the picker as a nested overlay inside `InstallWizardPanel`: a new
`widgets/install_wizard/version_picker.rs` with channel tabs (Stable / Beta / Master·git-only), a
scrollable `version · date · arch` list with a "git-only" badge, loading/error states, and footer
hints; hook it into the panel render; advertise the picker on the FlutterSdk step
(`step_detail.rs` action hint) and in the wizard footer.

**Depends on**: Task 02 (`VersionPickerState` / `PickerRow` / `PickerChannel` / `PickerFetch`).
Runs in parallel with Task 03 (write-disjoint: tui vs app).

**Agent:** implementor

**Complexity:** medium

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs` — **NEW**
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` — module decl; render hook; footer hint
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` — FlutterSdk caption/hint updates

**Files Read (Dependencies):**
- `fdemon-app` `install_wizard::version_picker` state types (Task 02)
- `crates/fdemon-tui/src/widgets/flutter_version_panel/version_list.rs` — `corrected_scroll`
  viewport pattern (~lines 235–265), row styling (selection ACCENT bg / focused conventions)
- `crates/fdemon-tui/src/widgets/modal_overlay.rs` — `centered_rect` / `clear_area`
- `crates/fdemon-tui/src/widgets/confirm_dialog.rs` — nested-modal precedent (Clear + Block, no
  second `dim_background`)

### Details

> Locate by symbol; line numbers drift.

#### 1. Widget (`version_picker.rs`, NEW)

A `Widget`-implementing struct borrowing `&VersionPickerState` (the `InstallWizardPanel` field
pattern). Render sequence inside `InstallWizardPanel::render`, **after** the panel body so it sits on
top (only when `state.version_picker.visible`):

- Area: `modal_overlay::centered_rect(width, height, dialog_area)` sized relative to the wizard
  dialog (e.g. min(60, dialog −4) × min(20, dialog −4)); reuse the wizard's min-size guard idiom for
  tiny terminals (skip render below the floor — the intercepted keys still work).
- `clear_area` + rounded `Block` titled `" Flutter version "` with `POPUP_BG` (confirm_dialog
  pattern — **no** second `dim_background`).
- Layout (vertical): tabs row (1) / separator (1) / list (Min) / footer (1).
- **Tabs row**: ` Stable │ Beta │ Master (git) ` — active tab ACCENT bold, others TEXT_MUTED.
- **List rows**: `▸ 3.24.0   2024-08-06   x64` — version (TEXT_BRIGHT; selected row ACCENT bg +
  CONTRAST_FG + BOLD per version_list conventions), `release_date` truncated to the date part
  (`&s[..10]` guarded), arch when `Some`, ` git-only` badge (STATUS_YELLOW) for `git_only` rows.
  Write `last_known_visible_height` first, then the render-time `corrected_scroll` slice — copy
  `version_list.rs` verbatim-in-spirit (no state mutation in render).
- **States**: `Loading` → centered "Fetching Flutter releases…" (wizard `render_loading` idiom);
  `Failed` → error text (wrapped, STATUS_RED) + `"Enter installs the default channel · r retries"`;
  `Loaded` + empty active tab → "No releases" muted line.
- **Footer**: `"[j/k] move · [Tab] channel · [Enter] install · [r] refetch · [Esc] close"`
  (TEXT_MUTED, `·` separators).

#### 2. Panel hook (`mod.rs`)

- `mod version_picker;` + render call at the end of `InstallWizardPanel::render` (after footer),
  gated on `self.state.version_picker.visible`.
- Footer hint augmentation: when the **selected step is FlutterSdk** and the picker is closed, append
  `"· [v] versions"` (the existing Platforms-parent conditional-append precedent, ~mod.rs:346-373).

#### 3. `step_detail.rs` — FlutterSdk arms

- `step_caption`: FlutterSdk gains
  `Some("  Enter chooses a version to install · v opens the version picker")` (today it is `None`).
- `action_hint_text`: keep `"▶ Press Enter to install Flutter SDK"` — but if the hint can reflect a
  confirmed pick cheaply (the pane already receives `&InstallWizardState`), prefer
  `"▶ Press Enter to install Flutter <version>"` via a small dynamic branch beside
  `render_action_hint` (don't contort the `&'static str` table: add a targeted override where the
  hint is rendered, leaving the table as the fallback).

### Acceptance Criteria

1. With `visible: false` nothing new renders (buffer identical to pre-task snapshot for a fixture
   state, modulo the new FlutterSdk caption/footer hint).
2. `Loaded` fixture (3 stable / 1 beta / master tab): tabs render with Stable active; rows show
   version + date + arch; switching the state's tab to Master renders the git-only badge; the
   selected row carries the selection style.
3. Scroll: with height 3 and cursor at index 5, the rendered slice contains the cursor row
   (corrected-scroll behaviour); `last_known_visible_height` is written by render.
4. `Loading` and `Failed` states render their messages; `Failed` shows the default-channel fallback
   hint.
5. FlutterSdk detail pane shows the new caption; wizard footer shows `[v] versions` on the FlutterSdk
   step only (picker closed).
6. `cargo test -p fdemon-tui --lib` green; fmt + clippy clean.

### Testing

```bash
cargo test -p fdemon-tui --lib widgets::install_wizard
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

House test pattern: `Buffer::empty(Rect::new(0,0,100,40))` + `widget.render` + symbol-collect +
`contains` assertions; per-row scans for anchoring (footer test precedent, mod.rs:665-696). Build
fixture `VersionPickerState`s directly via Task 02's methods (`apply_manifest` with a literal
manifest).

### Notes

- **Read-only over Task 02's state** — no new fields; if rendering needs something, flag it rather
  than mutating render-side (the `Cell` render-hint is the only sanctioned write).
- Truncation: long version strings (e.g. `1.12.13+hotfix.5`) must not panic on narrow widths — use
  the saturating/width-guard idioms from `version_list.rs`.
- Date display: raw ISO prefix is fine (`2024-08-06`); no chrono formatting, no new deps.
- Master rows have `release_date: None` / `arch: None` — render gaps, not "null".

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-aa63fd491dab79635

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs` | NEW — `VersionPickerOverlay` widget: channel tabs, scrollable list with corrected_scroll, Loading/Failed/Loaded states, git-only badge, footer hints; 18 tests |
| `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | Added `mod version_picker;`; render call gated on `version_picker.visible` at end of `InstallWizardPanel::render`; footer hint `[v] versions` when FlutterSdk selected + picker closed; 4 new tests |
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | FlutterSdk `step_caption` added; caption rendered above action hint for executable steps with no guided commands; dynamic Enter hint shows confirmed version; updated 1 existing test; added 3 new tests |

### Notable Decisions/Tradeoffs

1. **Picker max width raised to 80**: The footer hint string is 72 chars. The original `min(60, …)` cap would always clip "[Esc] close" from the footer. Raised to 80 (inner = 78) to fit the full hint. Still bounded, still consistent with the wizard's dialog.

2. **FlutterSdk caption rendered above action hint**: The task adds `step_caption` for FlutterSdk, but `render_guided_commands` (which normally renders captions) is never called for executable steps with no guided commands. Added a targeted `has_step_caption` path that reserves an extra row and renders the caption at `bottom_y`, shifting the action hint to `bottom_y + 1`. No layout disruption for other steps.

3. **Test for footer width**: The `test_footer_shows_all_hints` test uses a 160-wide area to ensure the 72-char footer isn't truncated (picker max width = 80, so inner = 78 ≥ 72).

4. **`step_caption` for FlutterSdk now used in two places**: in the new bottom-section caption renderer AND in `guided_section_full_height` (which returns 0 early for empty guided commands, so no change in behaviour for FlutterSdk's guided-command height).

### Testing Performed

- `cargo test -p fdemon-tui --lib widgets::install_wizard` — 165 passed, 0 failed
- `cargo test --workspace --lib` — 3056+514+1236+842+1530 = 7178 passed, 0 failed
- `cargo fmt --all -- --check` — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — clean

### Risks/Limitations

1. **Acceptance criteria 5 — caption in detail pane**: The FlutterSdk caption ("Enter chooses a version to install · v opens the version picker") is rendered one row above the action hint, inside the bottom section. In very short terminals (< 12 rows for the wizard) the bottom section may be squeezed to 0 rows and neither caption nor hint will show — consistent with the existing behavior for all other steps.
