## Task: Correct the ARCHITECTURE.md "toolchain display types" note (F2 doc)

**Agent:** doc_maintainer

**Severity:** MINOR

**Objective**: Update the `docs/ARCHITECTURE.md` note that enumerates the daemon toolchain
display types re-exported through `fdemon-app::install_wizard` so it reflects the addition
of `LinuxPackageManager` (task 03).

**Depends on**: 03-reexport-linux-package-manager (the doc must describe the final
re-export set)

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content-boundary rules.
- `crates/fdemon-app/src/install_wizard/mod.rs` — the final re-export block (after task 03).

### Change Context

`docs/ARCHITECTURE.md` (the "Note on daemon display types" under the `fdemon-tui` section,
~line 665) states: *"The four toolchain display types needed by the install-wizard widgets
(`ComponentCheck`, `ComponentStatus`, `DoctorLine`, `DoctorMarker`) are re-exported through
`fdemon-app::install_wizard` …"*.

After task 03, `LinuxPackageManager` is also re-exported and consumed by the install-wizard
TUI tests — and the actual re-export block already includes `HostPlatform`, `HostShell`,
and `ToolchainReport` beyond the "four". The "four" count is stale.

**Fix:** Correct the note so the count/enumeration is accurate — either:
- update the number and list to match the actual re-exported set (including
  `LinuxPackageManager`), or
- generalize the wording (e.g. "the daemon toolchain display types … are re-exported
  through `fdemon-app::install_wizard`, so presentation widgets never reach into
  `fdemon-daemon` directly") so it does not hard-code a count that drifts.

Prefer the generalized wording to avoid future staleness, but keep it accurate to the
established re-export-gateway pattern.

### Acceptance Criteria

1. The ARCHITECTURE.md display-types note no longer says "four" when the re-exported set is
   larger; it accurately reflects that `LinuxPackageManager` (and the other re-exported
   types) reach the TUI via `fdemon-app::install_wizard`.
2. Targeted edit only — no rewrite; no content-boundary violations; only `ARCHITECTURE.md`
   is edited.

### Notes

- `doc_maintainer` owns `ARCHITECTURE.md`; follow content boundaries strictly.
- Purely a note-accuracy fix; no module/layer/data-flow change was introduced by this
  followup.
