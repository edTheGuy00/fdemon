# Phase 4: Settings Panel Redesign - Task Index

## Overview

Transform the full-screen Settings panel to match the Cyber-Glass design, replicating `tmp/redesign/settings-page-focus.tsx`. The panel gets a glass container style, pill-style tab bar, icon+uppercase group headers, 3-column setting rows with left accent bar selection indicator, themed info banners, empty states, and a footer with icon-enhanced shortcut hints.

**Total Tasks:** 7
**Crate:** `fdemon-tui` (rendering + theme)
**Depends on:** Phase 1 (theme module)

## Task Dependency Graph

```
┌───────────────────────────────┐
│  01-add-settings-icons        │
│  (zap, eye, code, user,      │
│   keyboard, save to IconSet)  │
└───────────────┬───────────────┘
                │
                ├──────────────────────────────────┐
                ▼                                  ▼
┌───────────────────────────────┐  ┌───────────────────────────────┐
│  02-update-settings-styles    │  │  03-redesign-settings-header  │
│  (styles.rs → new design      │  │  (icon+title, pill tabs,      │
│   tokens, new style fns)      │  │   [Esc] close hint)           │
└───────────────┬───────────────┘  └───────────────┬───────────────┘
                │                                  │
                └─────────────┬────────────────────┘
                              ▼
              ┌───────────────────────────────┐
              │  04-redesign-settings-content  │
              │  (group headers with icons,    │
              │   3-col rows, accent bar)      │
              └───────────────┬───────────────┘
                              │
                ┌─────────────┴─────────────┐
                ▼                           ▼
┌───────────────────────────────┐  ┌───────────────────────────────┐
│  05-redesign-special-views    │  │  06-redesign-settings-footer  │
│  (user info banner, launch    │  │  (icon + key + description    │
│   empty state, vscode states) │  │   shortcut hints)             │
└───────────────┬───────────────┘  └───────────────┬───────────────┘
                │                                  │
                └─────────────┬────────────────────┘
                              ▼
              ┌───────────────────────────────┐
              │  07-update-tests              │
              │  (fix broken, add new)        │
              └───────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-add-settings-icons](tasks/01-add-settings-icons.md) | Not Started | - | `theme/icons.rs` |
| 2 | [02-update-settings-styles](tasks/02-update-settings-styles.md) | Not Started | 1 | `widgets/settings_panel/styles.rs` |
| 3 | [03-redesign-settings-header](tasks/03-redesign-settings-header.md) | Not Started | 1 | `widgets/settings_panel/mod.rs` |
| 4 | [04-redesign-settings-content](tasks/04-redesign-settings-content.md) | Not Started | 2, 3 | `widgets/settings_panel/mod.rs`, `styles.rs` |
| 5 | [05-redesign-special-views](tasks/05-redesign-special-views.md) | Not Started | 4 | `widgets/settings_panel/mod.rs` |
| 6 | [06-redesign-settings-footer](tasks/06-redesign-settings-footer.md) | Not Started | 4 | `widgets/settings_panel/mod.rs` |
| 7 | [07-update-tests](tasks/07-update-tests.md) | Not Started | 3, 4, 5, 6 | `widgets/settings_panel/tests.rs` |

## Execution Strategy

**Wave 1**: Task 01 — Add missing icon methods to `IconSet`. This is the foundation: all subsequent tasks reference these icons.

**Wave 2** (parallel): Tasks 02 and 03 — Style function updates (styles.rs) and header redesign (mod.rs). These modify different files and can run in parallel.

**Wave 3** (after 02+03): Task 04 — Content area redesign (mod.rs). Depends on updated styles and header being in place since it follows the header in the file and uses style functions.

**Wave 4** (parallel, after 04): Tasks 05 and 06 — Special views (info banners, empty states) and footer redesign. These touch different function ranges in mod.rs (05 modifies tab-specific renderers at lines 435+, 06 modifies render_footer at lines 246-268). They can run in parallel if implementors are careful about file coordination, or sequentially for safety.

**Wave 5** (after all): Task 07 — Update all broken tests and add new coverage.

## Success Criteria

Phase 4 is complete when:

- [ ] `IconSet` has 6 new methods: `zap()`, `eye()`, `code()`, `user()`, `keyboard()`, `save()` with Unicode and NerdFonts variants
- [ ] Settings header shows `ICON_SETTINGS` + "System Settings" in `TEXT_BRIGHT` bold
- [ ] Tab bar renders pill-style tabs with `ACCENT` bg on active tab, `TEXT_SECONDARY` on inactive
- [ ] Tab labels are uppercase with number prefix: "1. PROJECT", "2. USER", "3. LAUNCH", "4. VSCODE"
- [ ] `[Esc] Close` hint renders in top-right with kbd badge style
- [ ] Setting groups have icon + uppercase category header in `ACCENT_DIM`
- [ ] Setting rows use 3-column layout: label (25), value (15), description (flex)
- [ ] Selected row has left accent bar (`▎` in `ACCENT`) + subtle `ACCENT` bg tint
- [ ] Unselected rows have transparent left border + no background
- [ ] Value coloring matches design: bool=green/red, number=accent, string=primary, enum=indigo, list=blue
- [ ] User tab info banner renders as glass box with `ACCENT` bg tint + `ACCENT_DIM` border
- [ ] Launch tab empty state renders with centered icon container + title + subtitle
- [ ] VSCode tab info banner and empty states use consistent glass styling
- [ ] Footer shows 4 shortcut hints with icons: Tab/j,k/Enter/Ctrl+S
- [ ] `Ctrl+S` hint uses `ACCENT` color (emphasized)
- [ ] `settings_panel/styles.rs` fully aligned with theme design tokens
- [ ] All existing settings functionality preserved (tab switching, editing, saving, override indicators)
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes with no warnings

## Notes

- **Single-file bottleneck**: Unlike Phase 3 where widgets were split across many files, Phase 4's changes are concentrated in `settings_panel/mod.rs` (898 lines). Tasks 03-06 all modify this file. The dependency graph serializes edits to avoid merge conflicts, but implementors should be aware that later tasks will see code modified by earlier ones.
- **Styles migration timing**: Task 02 updates `styles.rs` with new design-aligned functions. Tasks 03-06 use these functions. If styles need further adjustment during visual tasks, update them inline and note it in the completion summary.
- **No layout.rs changes**: The settings panel manages its own layout internally (3-row vertical: header/content/footer). No changes to the main `layout.rs` are needed.
- **Icon fallback behavior**: All new icons must have both Unicode and NerdFonts variants. Unicode variants should be recognizable ASCII/Unicode characters that work without Nerd Fonts installed.
- **Functionality preservation is critical**: The settings panel has complex editing, tab switching, override indicators, config loading, and read-only modes. None of these should regress.
