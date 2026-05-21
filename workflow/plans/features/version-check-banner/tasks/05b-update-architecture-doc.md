## Task: Update ARCHITECTURE.md for version_check module

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to register the new `fdemon-app::version_check` module and document the new startup-time background task. Also remove any architectural references to the deleted migration-nudge machinery.

**Depends on**: 01-version-check-module, 03-banner-refactor, 04-spawn-and-wire

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`: 
  - In the `fdemon-app` module description, add `version_check.rs` to the file list. Add a one-line description (e.g. "GitHub releases API client; called at TUI startup to surface a 'new version available' banner").
  - In the section that lists background tasks spawned at startup (alongside `spawn_tool_availability_check` and `spawn_bootable_device_discovery`), add `spawn_version_check`.
  - Remove any architectural references to `emit_migration_nudge`, `has_cached_last_device`, or `NudgeMode` (these symbols are deleted in task 03; if ARCHITECTURE.md mentioned them, they must go).
  - If ARCHITECTURE.md describes the `[behavior]` config section's fields by name, add `version_check` alongside `confirm_quit` and `auto_launch`.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: Content boundary rules.
- `crates/fdemon-app/src/version_check.rs` (from task 01): Source of truth for what the module does.
- `crates/fdemon-app/src/spawn.rs` (from task 04): Source of truth for the new spawn function.
- `docs/ARCHITECTURE.md` itself: Existing module-map structure to match style.

### Change Context

Implementation changes that require ARCHITECTURE.md updates:

1. **New module `fdemon-app::version_check`** (task 01): A new leaf module in `fdemon-app` that makes one HTTPS call to GitHub via `reqwest` (rustls-tls). Module map and dependency list need to reflect its existence.

2. **New background task `spawn_version_check`** (task 04): Added alongside the existing fire-and-forget startup tasks. If ARCHITECTURE.md enumerates startup-time async work, add this one.

3. **Deletion of migration-nudge symbols** (task 03): `emit_migration_nudge`, `NudgeMode`, `has_cached_last_device` are removed. Any architectural mention must be deleted (no historical residue).

4. **New external dependency `reqwest`** (task 01): If ARCHITECTURE.md has a "Key external crates" or similar section, `reqwest` should be listed there with its purpose (GitHub version check).

### Acceptance Criteria

1. `grep -n "version_check" docs/ARCHITECTURE.md` returns at least one match.
2. `grep -n "emit_migration_nudge\|NudgeMode\|has_cached_last_device\|show_migration_banner" docs/ARCHITECTURE.md` returns no matches.
3. The fdemon-app module map (existing structure) now includes `version_check.rs` alongside its peers.
4. The doc-standards schema check passes — content boundaries are respected (no implementation code, no configuration values; only architectural facts).

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`. ARCHITECTURE.md is for module structure, layer dependencies, data flow. **Not** for code snippets or configuration examples (those live in CONFIGURATION.md, updated separately in task 05a).
- Make targeted edits — do not rewrite entire sections. The existing module map structure for `fdemon-app` should grow by one line, not be replaced.
- If unsure whether a piece of content belongs in ARCHITECTURE.md, consult the Content Boundary Quick Reference in the doc-standards skill.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/version-check-banner

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale>

### Testing Performed

- doc-standards validation — Pending
- `grep -n "version_check" docs/ARCHITECTURE.md` — Pending (should match)
- `grep -n "emit_migration_nudge" docs/ARCHITECTURE.md` — Pending (should be empty)

### Risks/Limitations

1. **<Risk>**: <Description>
