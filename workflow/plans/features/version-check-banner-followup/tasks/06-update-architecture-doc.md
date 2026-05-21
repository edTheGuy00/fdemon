## Task: Update ARCHITECTURE.md for version-check cache + late-arrival gate

**Agent:** doc_maintainer

**Objective**: Reflect the version-check hardening changes from tasks 03 and 04 in `docs/ARCHITECTURE.md`: (1) note the new on-disk cache artifact at `<dirs::cache_dir()>/fdemon/version_check.json` in the "System artifacts" section (or equivalent), and (2) update any startup-sequence narrative to describe the late-arrival gate behavior so future readers understand why the handler arm has a conditional.

**Depends on**: 03-handler-late-arrival-gate, 04-version-check-hardening

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**

- `docs/ARCHITECTURE.md`: Targeted edits only — do not rewrite sections.
  - Locate the `version_check.rs` row in the `fdemon-app` Core Modules table (added by task 05b in the original feature). Update its description to mention the on-disk cache.
  - Locate the startup-sequence enumeration that includes `spawn_version_check` (added by task 05b). Add a note about the late-arrival gate behavior: messages arriving after `ui_mode` transitions away from `Startup`/`NewSessionDialog` are dropped.
  - If there is a "System artifacts" / "Runtime files" / "On-disk state" section, add the cache file path. If no such section exists, add a one-line note in the `fdemon-app` module description.
  - Confirm no migration-nudge symbols (`emit_migration_nudge`, `NudgeMode`, `show_migration_banner`) have re-appeared (sanity check — they should still be absent from the original feature's task 05b work).

**Files Read (Dependencies):**

- `~/.claude/skills/doc-standards/schemas.md`: Content boundary rules.
- `crates/fdemon-app/src/version_check.rs`: Source of truth for the post-task-04 cache logic.
- `crates/fdemon-app/src/handler/update.rs`: Source of truth for the post-task-03 late-arrival gate.
- `docs/ARCHITECTURE.md` itself: Existing structure to match.

### Change Context

Implementation changes from tasks 03 + 04 that require ARCHITECTURE.md updates:

1. **On-disk cache artifact** (task 04): A new JSON file at `<dirs::cache_dir()>/fdemon/version_check.json` containing `{ checked_at, latest }`. Per-user, not per-project. 24h TTL. This is a runtime artifact comparable to `.fdemon/settings.local.toml` and should be documented if the architecture doc tracks such things.

2. **Late-arrival gate** (task 03): `Message::NewVersionAvailable` is conditionally applied based on `ui_mode`. This is a TEA-pattern subtlety worth a sentence in the startup-sequence narrative — future contributors reading the handler arm will see the `if` and want to understand the rationale without spelunking the followup plan.

### Acceptance Criteria

1. `grep -n "version_check.json" docs/ARCHITECTURE.md` returns at least one match (the cache artifact is named).
2. `grep -n "late.arrival\|dropped.*ui_mode\|NewSessionDialog.*startup_notice" docs/ARCHITECTURE.md` returns at least one match indicating the late-arrival behavior is documented.
3. The `version_check.rs` row in the Core Modules table reflects the new responsibility (HTTP + cache).
4. No content-boundary violations: no code snippets, no config TOML values inline (those live in CONFIGURATION.md).
5. doc-standards schema check passes.

### Notes

- Follow content boundaries strictly per `~/.claude/skills/doc-standards/schemas.md`. ARCHITECTURE.md is for module structure, layer dependencies, runtime artifacts, and data flow. **Not** for code snippets, configuration examples, or implementation details.
- Make targeted edits — the goal is two or three sentences added, not a rewrite. The cache path mention is one line; the late-arrival gate note is one sentence.
- If the existing ARCHITECTURE.md has no "System artifacts" / "On-disk state" section and adding one feels heavyweight for a single new file, fold the cache mention into the `version_check.rs` row of the Core Modules table instead. Choose the path of least restructuring.
- This task depends on both 03 and 04 because both implementation tasks change the system architecture that the doc describes. Run after their merges land.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/version-check-banner-followup

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale>

### Testing Performed

- doc-standards validation — Pending
- `grep -n version_check.json docs/ARCHITECTURE.md` — Pending
- `grep -n "late.arrival\|ui_mode" docs/ARCHITECTURE.md` — Pending

### Risks/Limitations

1. **<Risk>**: <Description>
