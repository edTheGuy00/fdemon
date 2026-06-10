## Task: Documentation — ARCHITECTURE.md for the Windows leaf

**Objective**: Record the Phase 5 architecture in `docs/ARCHITECTURE.md`: the live Windows platform
leaf, the new daemon probe, the `VisualStudioCpp` component, and the non-blocking semantics.

**Depends on**: Tasks 01–04 (merged — document what landed, not the plan).

**Agent:** doc_maintainer

**Complexity:** low

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`

**Files Read (Dependencies):**
- Phase 5 task files 01–04 (+ their completion summaries) under
  `workflow/plans/features/toolchain-platforms-submenu/phase-5/tasks/`.
- The merged source: `crates/fdemon-daemon/src/toolchain/checks/windows.rs`,
  `toolchain/types.rs`, `crates/fdemon-app/src/install_wizard/state.rs` (Windows leaf section).
- `~/.claude/skills/doc-standards/schemas.md` — content boundaries.

### Details

Update the existing install-wizard / toolchain sections (where Phases 2–4 documented the Platforms
submenu, `checks/web.rs`, and `checks/ios.rs`) — extend, don't restructure:

1. **Daemon toolchain checks**: add `checks/windows.rs` to the checks list — Windows-host-gated
   `check_windows` probe; `vswhere.exe` two-gate query (any instance / instance with the
   `VC.Tools.x86.x64` + `VC.CMake.Project` components); pure `classify_vswhere_gates` classifier;
   `PROBE_TIMEOUT` + `kill_on_drop` + `strip_and_truncate` hardening.
2. **`ComponentKind`**: the enum now includes `VisualStudioCpp` (13 variants); present on Windows
   reports only.
3. **Install wizard**: the Windows leaf is live — detect + guided-only (winget/choco/modify-existing),
   Missing→Partial cap, never blocks `flutter_now_live` handback; host-gated at the
   `report.platform == HostPlatform::Windows` level so the leaf is absent elsewhere.
4. **Cross-crate detail-prefix contract**: note that the `"Visual Studio found"` detail prefix produced
   by `classify_vswhere_gates` is consumed by `windows_guided_commands` to select the modify-vs-install
   guidance (the one piece a future reader cannot discover from either file alone).

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` accurately reflects the merged Phase 5 code (verify symbol names against the
   source, not the task files).
2. All five platform leaves are now described consistently in one place; no stale "placeholder /
   later phase" wording remains for Windows.
3. Content stays within ARCHITECTURE.md boundaries (no build commands, no keybinding tables).

### Testing

```bash
# Prose-only change — sanity-check the referenced symbols exist:
grep -rn "VisualStudioCpp" crates/fdemon-daemon/src/toolchain/
grep -n "check_windows" crates/fdemon-daemon/src/toolchain/checks/mod.rs
```

### Notes

- `docs/CONFIGURATION.md` / `docs/KEYBINDINGS.md` need **no** changes — Phase 5 adds no config field
  and no keybindings (verify and state so in the completion summary).
- Runs in parallel with Task 06 (website docs) — write-disjoint.
