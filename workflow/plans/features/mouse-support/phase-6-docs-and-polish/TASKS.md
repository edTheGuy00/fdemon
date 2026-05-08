# Phase 6: Documentation & Polish — Task Index

## Overview

Phase 6 closes out the mouse-support feature by finishing the documentation surfaces enumerated in `PLAN.md` Phase 6, refreshing the public website to reflect that fdemon now supports mouse, and resolving the one polish carry-over deferred from Phase 5.5 (compact-vertical `NewSessionDialog` mouse coverage). No new mouse semantics ship in this phase — every behavioural surface (scroll routing, region registry, modal precedence, sub-modal gates, double-click) is already in place from Phases 1–5.5.

Concrete deliverables:

1. **`docs/MOUSE.md`** gains Phase 5 sections: `NewSessionDialog`, `ConfirmDialog`, `TagFilter` overlay, `LinkHighlight` badges, and Settings panel click semantics; the Future Work section drops items shipped in Phase 5; a new "Compact NewSessionDialog" callout documents the size threshold below which device-row clicks are not registered.
2. **`docs/ARCHITECTURE.md`** Mouse Region Registry section is verified end-to-end and gains a short "Modal precedence and sub-modal gates" subsection reflecting the renderer-level base-region suppression introduced in Phase 5.5 Task 01.
3. **`docs/CODE_STANDARDS.md`** gains a "Region Registry Pattern" subsection under Responsive Layout Guidelines, citing the `Cell<usize>` render-hint precedent (lines 434–476) as the approved exception this pattern shares.
4. **`docs/KEYBINDINGS.md`** gains a top-of-file note pointing readers at `MOUSE.md` for mouse interactions; **`docs/IDEAS.md`** strikes the "Mouse Support" entry from the Deferred Features list.
5. **`docs/CONFIGURATION.md`** is verified to match Phase 5/5.5 final behaviour for `enable_mouse` (no semantic changes expected — the existing copy at lines 316/328/334 is already accurate; this task is mostly read-and-confirm with any drift fixes).
6. **Website** is updated to reflect mouse support:
   - The keyboard-disparaging marketing copy (`introduction.rs:25` "Never reach for the mouse." and `home.rs:75` "Designed for power users who prefer the keyboard over the mouse.") is softened to keyboard-first messaging that does not actively disparage mouse use.
   - A new `/docs/mouse` page mirrors `docs/MOUSE.md` and is added to the docs sidebar nav.
   - `data.rs` gains a "Mouse Interactions" `KeybindingSection` so the existing `Keybindings` page surfaces mouse mappings without duplicating layout code.
   - `configuration.rs` gains an `enable_mouse` row; `architecture.rs` gains a short "Mouse Subsystem" mention pointing at the mouse docs page.
7. **Compact NewSessionDialog mouse hint**: the compact-vertical `TargetSelector` render path (40–69 wide × 20–21 tall) renders a one-line hint — e.g. `"Resize terminal for mouse"` or equivalent — when `enable_mouse` is true and the layout is compact. The hint is keyboard-style (status-bar dim text, no extra rows) and disappears in wider layouts where device-row regions register normally. Resize-up restores full mouse coverage automatically (no state change required — region registration is per-frame).

When Phase 6 lands, anyone reading the docs in isolation understands what mouse can do, how to disable it, where it works/does-not-work, and how the registry is implemented; the website matches; and IDEAS.md no longer claims mouse support is deferred.

**Total Tasks:** 10
**Estimated Hours:** ~8.0 hours

## Prerequisites

- Phases 1–5 plus 1.5 / 2.5 / 3.5 / 4.5 / 5.5 follow-ups must be merged on `feat/mouse-support`. All mouse code is the baseline; Phase 6 adds no new mouse handlers, no new `Message` variants, and no new region kinds.
- No new dependencies. The website continues to build with the existing Trunk + Tailwind pipeline; the website task adds Rust code only (no new icons, fonts, or assets).
- `docs/ARCHITECTURE.md` and `docs/CODE_STANDARDS.md` edits are routed to `doc_maintainer` per the planner's documentation-routing rule.

## Out of Scope

- New mouse semantics. No new clickable surfaces, no new dispatchers, no new message variants. If reviewers find a missing surface during Phase 6, file it as a Phase 7 follow-up.
- Drag-to-select, drag-to-resize, hover tooltips, right-click context menus. Listed as Future Enhancements in `PLAN.md`.
- Region-registry allocation benchmark mentioned in PLAN edge cases. Deferred until profiling indicates a hotspot; the existing `Cell::take` + `Vec::clear` pattern is sufficient for current frame rates.
- Project selector (`selector.rs`) mouse support. Listed in Future Enhancements; not in Phase 6 scope.

## Task Dependency Graph

```
                 ┌────────────────────────────────────────────────────────┐
                 │  All 10 tasks have disjoint write-file sets — single   │
                 │  parallel wave; no internal dependencies.              │
                 └────────────────────────────────────────────────────────┘

  ┌────┬────┬────┬────┬────┬────┬────┬────┬────┬────┐
  ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼
┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐
│ 01 ││ 02 ││ 03 ││ 04 ││ 05 ││ 06 ││ 07 ││ 08 ││ 09 ││ 10 │
│MOUS││ARCH││CODE││KEY+││CONF││WEB-││WEB-││WEB-││WEB-││TUI-│
│.md ││.md ││.md ││IDEA││.md ││MKT ││MOUS││KEYS││CFG+││CMPT│
│P5  ││fin ││rgn ││strk││ver ││copy││page││mse ││arch││hint│
└────┘└────┘└────┘└────┘└────┘└────┘└────┘└────┘└────┘└────┘
 impl  doc_  doc_  impl  impl  impl  impl  impl  impl  impl
       maint maint
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate / Area | Agent |
|---|------|--------|------------|------------|--------------|-------|
| 1 | [01-mouse-md-phase5-coverage](tasks/01-mouse-md-phase5-coverage.md) | Done | — | 1.25h | `docs/` | implementor |
| 2 | [02-architecture-md-mouse-finalize](tasks/02-architecture-md-mouse-finalize.md) | Done (CONCERN: criterion-1 modal list was stale; impl matched render/mod.rs reality) | — | 0.5h | `docs/` | doc_maintainer |
| 3 | [03-code-standards-region-registry-pattern](tasks/03-code-standards-region-registry-pattern.md) | Done | — | 0.75h | `docs/` | doc_maintainer |
| 4 | [04-keybindings-and-ideas-cross-link](tasks/04-keybindings-and-ideas-cross-link.md) | Done | — | 0.25h | `docs/` | implementor |
| 5 | [05-configuration-md-verify](tasks/05-configuration-md-verify.md) | Done | — | 0.25h | `docs/` | implementor |
| 6 | [06-website-marketing-copy-softening](tasks/06-website-marketing-copy-softening.md) | Done | — | 0.5h | `website/` | implementor |
| 7 | [07-website-mouse-docs-page](tasks/07-website-mouse-docs-page.md) | Done | — | 1.5h | `website/` | implementor |
| 8 | [08-website-keybindings-mouse-section](tasks/08-website-keybindings-mouse-section.md) | Done | — | 0.75h | `website/` | implementor |
| 9 | [09-website-configuration-and-architecture](tasks/09-website-configuration-and-architecture.md) | Done | — | 0.75h | `website/` | implementor |
| 10 | [10-compact-new-session-dialog-mouse-hint](tasks/10-compact-new-session-dialog-mouse-hint.md) | Done | — | 1.5h | `fdemon-tui` | implementor |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-mouse-md-phase5-coverage | `docs/MOUSE.md` | `workflow/plans/features/mouse-support/phase-5-dialogs-overlays/TASKS.md`, `workflow/plans/features/mouse-support/phase-5.5-followup/TASKS.md`, `crates/fdemon-app/src/handler/mouse/*.rs` (interaction reference) |
| 02-architecture-md-mouse-finalize | `docs/ARCHITECTURE.md` | `crates/fdemon-app/src/mouse_regions.rs`, `crates/fdemon-app/src/handler/mouse/mod.rs`, `crates/fdemon-tui/src/render/mod.rs` |
| 03-code-standards-region-registry-pattern | `docs/CODE_STANDARDS.md` | `crates/fdemon-app/src/mouse_regions.rs`, `crates/fdemon-app/src/state.rs` (`MouseRegionsCell`) |
| 04-keybindings-and-ideas-cross-link | `docs/KEYBINDINGS.md`, `docs/IDEAS.md` | `docs/MOUSE.md` (cross-reference target) |
| 05-configuration-md-verify | `docs/CONFIGURATION.md` (only if drift found) | `crates/fdemon-app/src/config/types.rs`, `crates/fdemon-tui/src/terminal.rs` |
| 06-website-marketing-copy-softening | `website/src/pages/home.rs`, `website/src/pages/docs/introduction.rs` | n/a |
| 07-website-mouse-docs-page | `website/src/pages/docs/mouse.rs` (NEW), `website/src/pages/docs/mod.rs` | `docs/MOUSE.md` (content source) |
| 08-website-keybindings-mouse-section | `website/src/data.rs` | `docs/MOUSE.md`, `docs/KEYBINDINGS.md` |
| 09-website-configuration-and-architecture | `website/src/pages/docs/configuration.rs`, `website/src/pages/docs/architecture.rs` | `docs/CONFIGURATION.md`, `docs/ARCHITECTURE.md` |
| 10-compact-new-session-dialog-mouse-hint | `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/tests.rs` (or sibling) | `crates/fdemon-app/src/state.rs` (`enable_mouse` accessor), `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` (compact threshold) |

### Overlap Matrix

Wave 1 (no internal dependencies): all 10 tasks.

| Task Pair | Wave | Shared Write Files | Isolation Strategy |
|-----------|------|--------------------|---------------------|
| 01 + any | 1 | None — `docs/MOUSE.md` is a unique writer | **Parallel (worktree)** |
| 02 + any | 1 | None — `docs/ARCHITECTURE.md` is a unique writer | **Parallel (worktree)** |
| 03 + any | 1 | None — `docs/CODE_STANDARDS.md` is a unique writer | **Parallel (worktree)** |
| 04 + any | 1 | None — `docs/KEYBINDINGS.md` and `docs/IDEAS.md` are unique writers | **Parallel (worktree)** |
| 05 + any | 1 | None — `docs/CONFIGURATION.md` is a unique writer (likely a no-write verify) | **Parallel (worktree)** |
| 06 + 07/08/09 | 1 | None — T06 writes `home.rs` + `introduction.rs`; T07 writes `mouse.rs` + `mod.rs`; T08 writes `data.rs`; T09 writes `configuration.rs` + `architecture.rs` (all disjoint) | **Parallel (worktree)** |
| 07 + 08/09 | 1 | None — `docs/mouse.rs`, `docs/mod.rs`, `data.rs`, `configuration.rs`, `architecture.rs` are all distinct files | **Parallel (worktree)** |
| 08 + 09 | 1 | None — `data.rs` vs `configuration.rs`/`architecture.rs` | **Parallel (worktree)** |
| 10 + any | 1 | None — TUI widget files are not touched by any doc/website task | **Parallel (worktree)** |

Notes on overlap analysis:

- **Doc tasks 01–05 each write a unique file.** No two doc tasks touch the same path.
- **Website tasks 06–09** write four disjoint sets: marketing pages (`home.rs`, `introduction.rs`), new mouse page + nav (`docs/mouse.rs`, `docs/mod.rs`), keybindings data (`data.rs`), and config+architecture pages (`docs/configuration.rs`, `docs/architecture.rs`). Each task touches a different surface area.
- **Task 10 is fully isolated** in `crates/fdemon-tui/src/widgets/new_session_dialog/`; no website or doc task writes there.
- **`docs/MOUSE.md` is read by Tasks 04, 07, 08** — read-only overlap is acceptable per the Worktree-Aware Task Design rules. Each consumer task takes a snapshot of MOUSE.md content and embeds it; if Task 01 lands first and reshuffles section headings, the consumer tasks may need a small follow-up edit, but the writes remain disjoint.
- **`docs/ARCHITECTURE.md` is read by Task 09** for the architecture-page paragraph; same read-only pattern.
- **`docs/CONFIGURATION.md` is read by Task 09** for the `enable_mouse` row; same read-only pattern. If Task 05 finds drift and edits CONFIGURATION.md, Task 09 should pull from the post-edit version. To avoid a sequencing dependency, T09 only quotes the existing setting name + default, not the prose — so it remains correct regardless of T05's outcome.
- **No `Cargo.toml` edits.** No new dependencies. No new bin / lib targets.
- **No new `Message` variants, no new mouse handlers, no new `MouseAction` variants.** Task 10 (compact hint) is render-only — it adds a `Paragraph` line to the compact `TargetSelector` render path; no state changes, no message routing.

## Success Criteria

Phase 6 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes (existing suite + ≥ 1 new test for Task 10's hint render)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] **`docs/MOUSE.md`** has sections covering NewSessionDialog, ConfirmDialog, TagFilter overlay, LinkHighlight badges, and Settings panel click semantics; the modal-precedence and sub-modal-gate rules are documented; the Future Work list excludes anything shipped in Phase 5; the compact NewSessionDialog hint is documented under Platform / Compatibility caveats.
- [ ] **`docs/ARCHITECTURE.md`** Mouse Region Registry subsection has a verified-current "Modal precedence" paragraph reflecting the Phase 5.5 renderer-level base-region suppression. (Re-render-check by `doc_maintainer` against `crates/fdemon-tui/src/render/mod.rs`.)
- [ ] **`docs/CODE_STANDARDS.md`** has a "Region Registry Pattern" subsection under Responsive Layout Guidelines, citing the `Cell<usize>` render-hint precedent. The TEA exception note format matches the existing `// EXCEPTION:` comment style.
- [ ] **`docs/KEYBINDINGS.md`** has a top-of-file callout pointing at `docs/MOUSE.md`. **`docs/IDEAS.md`** no longer lists "Mouse Support" under Deferred Features (the entry, including its Priority/Complexity/Why-Deferred subsections, is removed; subsequent numbered entries are renumbered or the numbering stays as-is per existing project convention — match what the rest of IDEAS.md does).
- [ ] **`docs/CONFIGURATION.md`** `[ui] enable_mouse` row matches actual Phase 5/5.5 runtime behaviour (no drift). If drift is found, the row is corrected.
- [ ] **Website builds clean with `trunk build` (or the project's existing build command).** No new compiler warnings.
- [ ] **Website marketing copy** does not contain phrases that disparage mouse use (`"never reach for the mouse"`, `"prefer the keyboard over the mouse"`); replacement copy keeps the keyboard-first identity (e.g. `"Keyboard-first ergonomics, with optional mouse"`) without claiming mouse is unsupported.
- [ ] **Website docs sidebar** has a "Mouse" entry between "Keybindings" and "DevTools" (or position chosen by Task 07 author — placement chosen for navigability, not strict ordering).
- [ ] **Website Keybindings page** renders a "Mouse Interactions" section listing the same scroll/click table that lives in `docs/MOUSE.md`. The section uses the existing `KeybindingSection`/`Keybinding` structs; no new layout code.
- [ ] **Website Configuration page** documents `enable_mouse` next to the other `[ui]` settings, with a one-line cross-reference to the new `/docs/mouse` page.
- [ ] **Website Architecture page** has a short "Mouse Subsystem" paragraph mentioning the registry pattern with a link to the mouse docs page.
- [ ] **Compact `NewSessionDialog`** (40–69 wide × 20–21 tall, where the layout falls back to compact-vertical `TargetSelector`) renders a hint line — `"Resize for mouse"` or equivalent — when `state.settings.ui.enable_mouse` is `true`. The hint disappears when the layout exits compact mode (resize-up). The hint is **never shown** when `enable_mouse = false`. Verified by widget snapshot test at three terminal sizes (60×20 compact, 100×30 wide, any-width with `enable_mouse=false`).
- [ ] **Manual smoke test (macOS):**
  - Run fdemon, resize terminal to 50 × 20 → `NewSessionDialog` enters compact-vertical mode → hint line is visible.
  - Resize back to 100 × 30 → hint disappears, device rows are clickable as before.
  - Set `enable_mouse = false` in `.fdemon/config.toml`, restart, repeat the resize → hint is never shown at any size.
  - Visit the deployed (or locally-served) website, confirm: marketing copy is neutral; `/docs/mouse` page renders; keybindings page has the new section; configuration page mentions `enable_mouse`.

## Notes

- **Why route ARCHITECTURE.md and CODE_STANDARDS.md to `doc_maintainer`.** Per `~/.claude/skills/planner/templates.md` and the planner skill's Documentation Update Requirements, these two documents have strict content boundaries enforced by the `doc_maintainer` agent. The implementor is not permitted to edit them; the orchestrator must dispatch Tasks 02 and 03 to `doc_maintainer` (signaled by the `Agent: doc_maintainer` line in each task file). MOUSE.md, KEYBINDINGS.md, IDEAS.md, and CONFIGURATION.md are unmanaged docs and may be edited by the implementor.
- **Why a single Phase 6 instead of a Phase 6 + Phase 6.5 split.** Phase 6 has no critical defects to chase down; the Phase 5.5 carry-overs (compact dialog hint) are scoped tightly enough to land alongside the docs work. A 6.5 follow-up would only add ceremony.
- **Why the compact-dialog *hint* approach over a full region-recording implementation.** Per the planner's clarifying question: implementing compact-vertical regions duplicates ~50% of Phase 5 Task 09's logic for a code path users rarely hit (terminal narrower than 70 columns). The hint approach keeps Phase 6 small, signals the limitation clearly, and leaves the door open for a future Phase 7 task if user feedback indicates demand. The compact dialog remains fully usable via keyboard — there is no functional regression, only an absent ergonomic.
- **Why the website surfaces the mouse content rather than just linking the markdown.** The website is a marketing + getting-started surface; readers there expect inline content. Linking `docs/MOUSE.md` (a GitHub-rendered file) breaks the SPA experience. Tasks 07–09 keep the website as a self-contained reference that mirrors the canonical markdown.
- **Why no benchmark.** Phase 5.5 shipped with no observed allocation hotspot in the registry; the `Cell::take` + `Vec::clear` pattern reuses storage. The PLAN edge case ("Verified with a benchmark in Phase 6") is a soft mitigation against a risk that has not materialised. Adding a benchmark requires standing up a `benches/` directory + `criterion` dev-dep, which is disproportionate. If a future profile shows registry churn, file a Phase 7 follow-up.
- **Compact-dialog hint copy.** Task 10 may pick the exact wording. Suggested forms: `"Resize for mouse"` (5 words / fits 60-col) or `"⌨ keyboard only"` (Unicode keyboard glyph + label). Author chooses; the dim-style status row should not push other content out of view.
- **No new `Message` variants, no new `MouseAction` cases, no new dispatchers.** Phase 6 is documentation + a render-only hint. The only `crates/` write is `target_selector.rs`.
- **Website build verification.** `website/Trunk.toml` and `website/Cargo.toml` already exist; `trunk build` is the canonical command. Tasks 06–09 must run it locally (or the project's documented equivalent) before marking complete.
- **Doc cross-links assume URL paths.** `docs/KEYBINDINGS.md` cross-links to `MOUSE.md` (relative path), and the website's `docs/keybindings.rs` cross-links to `/docs/mouse` (SPA route). These are different reference styles intentionally.
