## Task: Update KEYBINDINGS docs for Prerequisites guided commands + per-command nav

**Agent:** doc_maintainer

**Objective**: Document the Phase-4 wizard changes — the new `[`/`]` per-command
navigation keys, the now index-aware `c` copy, and that the Prerequisites step is a
guided (non-executable) step with copyable per-OS install commands re-checked via `r`.

**Depends on**: 04-per-command-navigation, 05-tui-prereq-detail-render

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `docs/KEYBINDINGS.md`: Install Wizard Mode section.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: content boundary rules.
- Task files `03-prerequisites-guided-commands.md`, `04-per-command-navigation.md`
  for change context.

### Change Context

1. **New keys** `[` / `]` select the previous / next guided command on the current
   wizard step (used when a step shows multiple commands, e.g. macOS Prerequisites:
   Xcode CLT / CocoaPods / Rosetta).
2. **`c` is now index-aware** — it copies the *selected* guided command, not just
   the first.
3. **Prerequisites is a guided step** — `Enter` is not executable; the user copies
   a per-OS install command (Linux package-manager command; macOS CLT/CocoaPods/
   Rosetta; Windows Git for Windows), runs it manually, then presses `r` to
   re-check. This is distinct from executable steps like FlutterSdk/PathConfig.

### Acceptance Criteria

1. The Install Wizard Mode keybindings table lists `[` and `]` with accurate
   descriptions and updates the `c` entry to "copy the **selected** guided command".
2. The surrounding prose notes that the Prerequisites step is guided-only and that
   per-OS prerequisite commands (e.g. the package-install command) are copyable.
3. No content-boundary violations; only `KEYBINDINGS.md` is edited.

### Notes

- **Only `KEYBINDINGS.md` changes.** No new config options → do **not** touch
  `CONFIGURATION.md`. No new module/layer/data-flow → no `ARCHITECTURE.md` change is
  warranted for Phase 4 (detection refinement and guided-command derivation live in
  existing files).
- The existing `c`/`r` entries in the Install Wizard Mode section already use the
  JDK command as an example — extend them to also mention OS prerequisites.
- Follow content boundaries strictly; make targeted edits, do not rewrite the doc.
