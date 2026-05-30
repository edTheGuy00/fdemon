# Phase 7: Multi-session Header Spacing + Separator — Task Index

## Overview

A single, self-contained TUI-only fix for the multi-session header block.

**Problem (confirmed by research):** In multi-session mode the header is allocated
`Constraint::Length(5)` (`layout.rs:27`) → 3 inner rows after the rounded `glass_block`
border. `render_main_header` (`widgets/header.rs:128-149`) only paints 2 of them — title
at `inner.y`, tabs at `inner.y + 1` — leaving the 3rd inner row (`inner.y + 2`) as blank
`CARD_BG` **inside** the border (the "too much empty space at the bottom"), while the
title and tabs sit adjacent with no separation. The `glass_block` has no `Padding`
(`theme/styles.rs:102`), so spacing must come from explicit row placement.

**Fix:** Re-layout the existing 3 inner rows as **title → dim `BORDER_DIM` separator rule
→ tabs**. `header_height` stays 5 (no layout-height change, `layout.rs` tests untouched);
the only code change is in `widgets/header.rs` plus a comment reword in `layout.rs`.

**Total Tasks:** 1
**Estimated Hours:** 1–1.5h

## Task Dependency Graph

```
┌──────────────────────────────────────────────┐
│ 01-header-separator-spacing                    │
│   fdemon-tui/widgets/header.rs (re-layout)     │
│   fdemon-tui/layout.rs (comment only)          │
└──────────────────────────────────────────────┘
                 (single task)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-header-separator-spacing](tasks/01-header-separator-spacing.md) | ⬜ Todo | - | 1–1.5h | `crates/fdemon-tui/src/widgets/header.rs`, `crates/fdemon-tui/src/layout.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-header-separator-spacing | `crates/fdemon-tui/src/widgets/header.rs`, `crates/fdemon-tui/src/layout.rs` | `crates/fdemon-tui/src/theme/palette.rs` (`BORDER_DIM` — read), `crates/fdemon-tui/src/theme/styles.rs` (`glass_block` — read), `crates/fdemon-tui/src/widgets/tabs.rs` (`render_session_tabs` padding — read) |

### Overlap Matrix

Single task — no wave-peers, no overlap. Run on its own branch/worktree.

**Waves:** Wave 1 = `01` alone.

**Cross-phase note:** `header.rs` is also touched by Phase 6 (reload-flash `bg` blend) and
Phase 6.5 (status-bar spinner lives in `log_view/mod.rs`, not here). Phase 6 is already
merged; this task **builds on** the existing flash `bg` (it must thread the same `bg` into
the new separator rule, see task). No live conflict — sequence after Phase 6 if not yet
merged.

## Success Criteria

Phase 7 is complete when:

- [ ] In multi-session mode the header renders title → dim `BORDER_DIM` separator rule →
      device tabs, with **no** empty `CARD_BG` band at the bottom of the bordered block.
- [ ] The separator rule is inset 1 cell on each side (aligned with the tabs' existing
      `x+1 / width-2` padding) and tints with the reload-flash `bg` like the rest of the
      header.
- [ ] `header_height` is unchanged (still 5); `layout.rs` tests stay green and only the
      inline comment is reworded.
- [ ] The vertically-squeezed fallback (`inner.height == 2`) still renders title + tabs
      with no separator and without clipping; single-session / no-session headers are
      visually unchanged.
- [ ] A `header.rs` render test asserts the rule glyph on `inner.y + 1` and tabs content
      on `inner.y + 2`.
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Notes

- **No new state, no new config, no keybindings, no managed-doc change.** No `AppPhase` /
  `Message` / module-structure / layer-dependency change, so `docs/ARCHITECTURE.md`,
  `docs/CODE_STANDARDS.md`, and `docs/DEVELOPMENT.md` need no update.
- **Height is deliberately unchanged.** Keeping `header_height = 5` avoids reflowing the
  log area and keeps every `layout.rs` test valid; the fix only re-distributes rows that
  were already allocated.
- **Separator must respect the reload-flash `bg`.** `render_main_header` already computes
  a flash-blended `bg` (`header.rs:104`); the new rule paints `BORDER_DIM` fg on **that**
  `bg`, not a hard-coded `CARD_BG`, so a reload flash tints the whole block uniformly.
