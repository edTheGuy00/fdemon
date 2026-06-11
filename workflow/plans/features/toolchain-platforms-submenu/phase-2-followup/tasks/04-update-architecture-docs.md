## Task: Update Documentation for the directional expand/collapse messages (Phase 2 followup)

**Agent:** doc_maintainer

**Objective**: Refresh `docs/ARCHITECTURE.md` so the install-wizard description reflects the directional
`InstallWizardExpand` / `InstallWizardCollapse` messages and their handlers added in Task 01, alongside the
existing `InstallWizardToggleExpand`. Keep it architecture-level (message categories + handler
responsibilities); the per-key bindings live in `docs/KEYBINDINGS.md` (updated by Task 01, not here).

**Depends on**: 01-navigation-correctness

**Estimated Time**: ~0.5 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — the install-wizard message list + `navigation.rs` handler description.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- `crates/fdemon-app/src/message.rs` — final `InstallWizardExpand`/`InstallWizardCollapse` variants.
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` — `handle_expand`, `handle_collapse`, and the
  shared `set_platforms_expanded` helper (the single rebuild/re-anchor/clamp/reset path).
- `workflow/plans/features/toolchain-platforms-submenu/phase-2-followup/tasks/01-navigation-correctness.md`
  for change context.

### Change Context

Phase 2 followup made the Platforms submenu expand/collapse directional and consolidated the cursor logic:
- **Messages:** added `InstallWizardExpand` (set `platforms_expanded = true`) and `InstallWizardCollapse`
  (set `false`) next to the existing `InstallWizardToggleExpand` (flip, on the parent). `l`/`Right`→expand,
  `h`/`Left`→collapse, `Enter` on the parent→toggle.
- **Handlers:** `navigation.rs` gained `handle_expand` / `handle_collapse`, and a single private
  `set_platforms_expanded` helper now drives every transition (toggle, directional, Esc-collapse) — it
  rebuilds the projected step list, re-anchors the cursor to the `Platforms` parent when collapsing from a
  leaf row, clamps `selected_index`, and resets `selected_command_index`.

### Acceptance Criteria

1. The install-wizard message-category list in `docs/ARCHITECTURE.md` includes `InstallWizardExpand` and
   `InstallWizardCollapse` (alongside the existing `InstallWizardToggleExpand`), described at the
   architectural level.
2. The `navigation.rs` / install-wizard handler description notes the directional handlers and the single
   `set_platforms_expanded` transition helper (rebuild + cursor re-anchor + clamp + command reset). It need
   not enumerate key bindings (those are KEYBINDINGS.md's domain).
3. No content-boundary violations (architecture/structure only — no how-to, no key tables).
4. Targeted edits only; no unrelated sections rewritten; cross-references remain valid.
5. No stale claim that `l`/`h` merely "toggle" — the doc reflects the directional behavior.

### Notes

- Follow `~/.claude/skills/doc-standards/schemas.md` strictly.
- This is a small, targeted refresh of the section the Phase 2 doc task already wrote — extend it, don't
  rewrite it.
- `docs/KEYBINDINGS.md` is updated by Task 01 (implementor-editable); do not duplicate key tables here.

---

## Completion Summary

**Status:** _(fill in)_
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
