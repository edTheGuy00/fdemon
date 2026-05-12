## Task: Update Architecture Doc for DevTools Serve Flow

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` "DevTools Subsystem" to describe the new `daemon.devtools.serve` flow, the `Session.devtools_endpoint` field, and the fallback path.

**Depends on**: 07-fallback-and-recovery-toast

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` (around line 839, "DevTools Subsystem"): Add a subsection (or paragraph) describing:
  - On VM Service ready, fdemon fires `DaemonCommand::ServeDevTools`.
  - On response, `Session.devtools_endpoint` is populated.
  - `B` key uses the served endpoint; falls back to legacy URL if absent.
  - Minimum SDK note from RESEARCH.md.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: Content boundary rules.
- `workflow/plans/bugs/browser-devtools-dds-registration/RESEARCH.md`: For accurate version info.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` describes the new flow accurately.
2. No content boundary violations.
3. Minimum SDK version from RESEARCH.md is mentioned.
4. No code blocks or implementation details — pure architecture-level prose.

### Notes

- Targeted edits only — do not rewrite the DevTools subsystem section.
- If `RESEARCH.md` revealed a different method name than `daemon.devtools.serve`, use the verified name.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Added "Browser DevTools URL (Served Endpoint)" subsection to the DevTools Subsystem section (after "VM Service Data Flow") describing the two-channel endpoint acquisition flow, `Session.devtools_endpoint` storage, fallback path, recovery toast, and minimum SDK note (Flutter ≥ 1.22). |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: YES/NO/N/A: N/A

### Notable Decisions/Tradeoffs

1. **Placement after "VM Service Data Flow"**: The new subsection is the natural continuation of the VM Service connection narrative; inserting it before the closing `---` keeps the DevTools Subsystem self-contained.
2. **No code blocks**: All content is architecture prose only, in compliance with ARCHITECTURE.md boundary rules.
3. **Used dispatch-provided verified facts**: Method name `devtools.serve`, event `app.devTools`, min SDK Flutter ≥ 1.22, and `Session.devtools_endpoint` shape all sourced from the key corrections in the dispatch context rather than the (absent) RESEARCH.md file.
