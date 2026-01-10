## Task: Update Documentation

**Objective**: Update architecture and development documentation to reflect the new startup flow.

**Depends on**: 01-remove-dead-code

**Estimated Time**: 0.5 hours

### Scope

- `docs/ARCHITECTURE.md`: Update startup sequence diagram
- `CLAUDE.md`: Update if startup is mentioned

### Details

#### Update ARCHITECTURE.md

The "Startup Sequence" section (around line 389-400) describes the old flow:

**Current text:**
```markdown
### Startup Sequence

1. main.rs: Parse CLI args
2. main.rs: Check if path is runnable Flutter project
3. main.rs: If not, discover projects in subdirectories
4. main.rs: If multiple, show project selector
5. app::run_with_project(): Initialize logging
6. tui::run_with_project(): Initialize terminal
7. tui::run_with_project(): Load settings
8. tui::run_with_project(): Show device selector (if auto_start=false)
9. tui::run_with_project(): Spawn Flutter process
10. tui::run_loop(): Enter main event loop
```

**Updated text:**
```markdown
### Startup Sequence

1. main.rs: Parse CLI args
2. main.rs: Check if path is runnable Flutter project
3. main.rs: If not, discover projects in subdirectories
4. main.rs: If multiple, show project selector
5. app::run_with_project(): Initialize logging
6. tui::run_with_project(): Initialize terminal
7. tui::run_with_project(): Load settings
8. tui::startup_flutter(): Enter Normal mode (always)
9. tui::run_with_project(): Render first frame
10. tui::run_with_project(): If auto_start=true, send StartAutoLaunch message
11. tui::run_loop(): Enter main event loop
12. (If auto_start) Handler shows Loading, discovers devices, spawns session
```

#### Update Data Flow Section

If there's a diagram or description of auto-start flow, update it to show the message-based approach.

**Add/update:**
```markdown
### Auto-Start Flow (auto_start=true)

1. runner.rs sends Message::StartAutoLaunch after first render
2. Handler sets UiMode::Loading, returns DiscoverDevicesAndAutoLaunch action
3. Spawn function discovers devices asynchronously
4. Progress messages update loading screen
5. AutoLaunchResult message triggers session creation
6. Handler clears loading, returns SpawnSession action
7. Session starts, enters Normal mode with running session
```

#### Check CLAUDE.md

Review `CLAUDE.md` for any startup-related instructions that need updating. The file contains project guidance for Claude Code.

Look for:
- References to `startup_flutter()`
- References to auto-start behavior
- Any outdated startup flow descriptions

### Acceptance Criteria

1. ARCHITECTURE.md startup sequence updated
2. ARCHITECTURE.md data flow section updated (if applicable)
3. CLAUDE.md reviewed and updated if needed
4. Documentation accurately reflects new behavior
5. No broken markdown formatting

### Notes

- Keep documentation concise - don't over-document
- Focus on what developers need to know
- The plan document itself serves as detailed documentation
- Consider adding a link to the plan from ARCHITECTURE.md

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (pending)

**Changes Made:**

(pending)

**Notes:**

(pending)
