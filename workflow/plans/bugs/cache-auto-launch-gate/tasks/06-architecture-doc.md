# Task 06 — Update `docs/ARCHITECTURE.md` startup-sequence summary

**Plan:** [../BUG.md](../BUG.md) · **Index:** [../TASKS.md](../TASKS.md)
**Agent:** doc_maintainer
**Depends on:** Tasks 01, 02, 03, 04
**Wave:** 3 (parallel with Task 05)

## Goal

The Startup Sequence summary in `docs/ARCHITECTURE.md` (Data Flow section, line ~1444) describes the gate condition with a single bullet: "Show device selector (if auto_start=false)". Now that the gate has a second condition (`auto_launch` flag + valid cache), update the summary to remain accurate. This is the **only** ARCHITECTURE.md change required — module structure, layer dependencies, and data flow shapes are unchanged.

## Files Modified (Write)

| File | Change |
|------|--------|
| `docs/ARCHITECTURE.md` | Update line 1444 (and adjacent context lines if needed) so the Startup Sequence summary describes the new gate condition. Optionally add a short bullet that names the four-tier cascade or links to `docs/CONFIGURATION.md` for the full priority table. |

## Files Read (dependency)

- Tasks 01-04 (to describe shipped behavior accurately)
- `docs/CONFIGURATION.md` after Task 05 (the canonical source for the gate spec; ARCHITECTURE.md should defer rather than duplicate)

## Implementation Notes

### Current text (line 1432-1447)

```
## Data Flow

### Startup Sequence

```
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
```

### Suggested replacement

```
## Data Flow

### Startup Sequence

```
1. main.rs: Parse CLI args
2. main.rs: Check if path is runnable Flutter project
3. main.rs: If not, discover projects in subdirectories
4. main.rs: If multiple, show project selector
5. app::run_with_project(): Initialize logging
6. tui::run_with_project(): Initialize terminal
7. tui::run_with_project(): Load settings (config.toml + launch.toml + settings.local.toml)
8. tui::run_with_project(): Auto-launch gate — fires when launch.toml has auto_start=true,
   OR when [behavior] auto_launch=true AND a valid last_device is cached.
   Otherwise: show New Session dialog. (See docs/CONFIGURATION.md for the full priority table.)
9. tui::run_with_project(): Spawn Flutter process (if auto-launch fired)
10. tui::run_loop(): Enter main event loop
```
```

### Boundary check

- `docs/ARCHITECTURE.md` describes module structure and high-level data flow. The detailed priority table belongs in `docs/CONFIGURATION.md` (Task 05 owns it). ARCHITECTURE.md should reference it rather than duplicate.
- No changes to module structure, layer crossings, dependency graph, or any other ARCHITECTURE.md section.

## Verification

- Visual review (markdown preview).
- Confirm no other ARCHITECTURE.md sections reference the old `auto_start=false` shorthand. Search for `auto_start` and `auto_launch` to verify completeness.

## Acceptance

- [x] Line 1444 (and surrounding context as needed) reflects the new gate condition.
- [x] No other sections of ARCHITECTURE.md changed.
- [x] Reference to `docs/CONFIGURATION.md` for the full table.
- [x] No grep hits for stale "if auto_start=false" phrasing.

---

## Completion Summary

**Status:** Done
**Branch:** plan/cache-auto-launch-gate

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Updated Startup Sequence step 7 to list the three settings files loaded, and replaced step 8's single `auto_start=false` bullet with the two-condition auto-launch gate description plus a reference to `docs/CONFIGURATION.md`. Step 9 updated to note it only fires if auto-launch fired. |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: YES/NO/N/A: N/A

### Notable Decisions/Tradeoffs

1. **Deferred detail to CONFIGURATION.md**: The full four-tier priority table lives in `docs/CONFIGURATION.md` (Task 05). ARCHITECTURE.md references it rather than duplicating, keeping the startup sequence readable and the detail canonical in one place.
